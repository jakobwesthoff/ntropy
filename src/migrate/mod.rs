// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Whole-vault conversions (ADR 0041).
//!
//! `vault encrypt`, `vault decrypt` and `vault rekey` rewrite every note, so
//! they are built to be safe to interrupt. The shape is the same for all three
//! and rests on one property: **the conversion is purely additive until its
//! final step**.
//!
//! 1. **Produce.** Each converted note is written as a sibling of the note it
//!    came from. Nothing is removed, so at every instant the vault holds a
//!    complete copy of itself in at least one form. The produced files are
//!    flushed to stable storage together, then renamed into place, then their
//!    directory is flushed, so every source has a durable replacement before
//!    anything is removed.
//! 2. **Verify.** Every produced file is read back and compared against its
//!    source's plaintext. A mismatch aborts while both forms are still on disk.
//! 3. **Commit.** The sources are deleted, a rekey's produced files take their
//!    final names, the directory is flushed, and the marker is removed. This
//!    is the point of no return, and nothing before it loses information.
//!
//! `--resume` re-verifies by reading and comparing, never by checking that a
//! target exists: an existing file is not proof that it is right.

pub mod marker;
pub mod plan;
pub mod rekey;

use std::path::{Path, PathBuf};

pub use marker::{Marker, Operation};
pub use plan::Conversion;
pub use rekey::KeyHandover;

use crate::cipher::{CipherError, NoteCipher};
use crate::fsutil::{self, FsError};
use crate::keys::KeyError;
use crate::reconcile;
use crate::scan::{self, ScanError};
use crate::session::VaultSession;
use crate::vault::Vault;
use crate::view::{self, SkippedView};

/// Why a conversion could not run or complete.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error(transparent)]
    Fs(#[from] FsError),

    #[error(transparent)]
    Scan(#[from] ScanError),

    #[error(transparent)]
    Cipher(#[from] CipherError),

    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    #[error("`{}` is not a readable migration marker", .path.display())]
    MalformedMarker {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("`{}` records marker version {version}, which this ntropy does not understand", .path.display())]
    UnknownMarkerVersion { path: PathBuf, version: u32 },

    #[error(transparent)]
    Key(#[from] KeyError),

    /// A rekey's marker does not name the key it is converting to.
    #[error("the rekey marker records no valid target key")]
    MarkerWithoutRecipient,

    /// The key pair a rekey staged is not in `.ntropy/`.
    #[error("the staged key is missing: {}; the rekey cannot continue", quoted_list(.0))]
    StagedKeyMissing(Vec<PathBuf>),

    /// The staged key is not the one the marker names.
    #[error(
        "`{}` holds the key `{found}`, but the rekey in progress converts to `{expected}`; \
         nothing was changed",
        .path.display()
    )]
    StagedKeyMismatch {
        path: PathBuf,
        expected: String,
        found: String,
    },

    /// The staged identity did not open under the passphrase given.
    #[error(
        "`{}` could not be opened; it is wrapped under the passphrase the interrupted \
         rekey was given (`--new-passphrase-file` if it had one)",
        .path.display()
    )]
    StagedKeyLocked {
        path: PathBuf,
        #[source]
        source: KeyError,
    },

    /// Notes that the key expected to open them does not open.
    #[error(
        "these notes cannot be opened with the key they should be encrypted to: {}; \
         repair or remove them first",
        quoted_list(.0)
    )]
    UnreadableNotes(Vec<PathBuf>),

    /// Produced files whose note is gone, with no key on disk known to open them.
    #[error(
        "these files hold notes whose original is gone: {}; move them away, or remove them \
         if their key is lost, then start the rekey again",
        quoted_list(.0)
    )]
    StrandedNotes(Vec<PathBuf>),

    #[error("could not serialize the migration marker")]
    Serialize(#[from] toml::ser::Error),

    /// A produced note did not read back as its source.
    ///
    /// Raised before anything is deleted, so both forms survive for the user to
    /// inspect.
    #[error("`{}` did not read back as `{}`; nothing was deleted", .target.display(), .note.display())]
    VerificationFailed { note: PathBuf, target: PathBuf },

    /// A conversion was asked to resume with nothing in progress.
    #[error("no conversion is in progress in this vault")]
    NothingToResume,

    /// A conversion was started while another is unfinished.
    #[error("a `{operation}` is already in progress; finish it with `{resume}`")]
    AlreadyInProgress {
        operation: Operation,
        resume: &'static str,
    },

    /// A test asked the conversion to stop at a phase boundary.
    #[error("interrupted after the {0} phase")]
    Interrupted(Phase),

    /// Notes share a ULID with another note (ADR 0067). Converting would
    /// write both to one file, or leave the copies behind unconverted.
    #[error(
        "nothing was converted: these copies share a ULID with another note: {}; \
         run `ntropy reconcile` first, which gives each copy a ULID of its own",
        quoted_list(.0)
    )]
    DuplicateIds(Vec<PathBuf>),

    /// Files sit where the conversion would write notes, and would be
    /// overwritten (ADR 0067).
    #[error(
        "nothing was converted: the conversion would overwrite {}; {}",
        quoted_list(.paths),
        targets_exist_hint(.operation)
    )]
    TargetsExist {
        operation: Operation,
        paths: Vec<PathBuf>,
    },
}

/// `paths` as a comma-separated list of quoted file names.
fn quoted_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap_or(path.as_os_str());
            format!("`{}`", name.to_string_lossy())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// What to do about files a conversion would overwrite.
///
/// Before decrypting, such a file is a plaintext note dropped in next to the
/// encrypted one it collides with, which `reconcile` adopts under a ULID of
/// its own. In a plaintext vault a stray `.age` file is a resource that
/// `reconcile` leaves alone, so only moving it helps.
fn targets_exist_hint(operation: &Operation) -> &'static str {
    match operation {
        Operation::Decrypt => {
            "run `ntropy reconcile` first, which encrypts a plaintext note found there \
             under a ULID of its own, or move the files away"
        }
        Operation::Encrypt | Operation::Rekey => "move or remove these files first",
    }
}

/// The three phases of a conversion, as a value tests can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Produce,
    Verify,
    Commit,
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Phase::Produce => "produce",
            Phase::Verify => "verify",
            Phase::Commit => "commit",
        };
        f.write_str(name)
    }
}

/// A hook the conversion consults at each phase boundary.
///
/// The seam crash-safety tests inject a failure through: a test can stop a
/// conversion exactly between phases and then assert what survived on disk,
/// without a subprocess and without the library learning about environment
/// variables.
pub trait MigrationHooks {
    /// Called once every target has been produced, before verification.
    fn after_produce(&self) -> Result<(), MigrateError> {
        Ok(())
    }

    /// Called once every target has verified, before anything is deleted.
    fn after_verify(&self) -> Result<(), MigrateError> {
        Ok(())
    }

    /// Called after each source has been replaced, with the number replaced so
    /// far.
    fn after_commit_step(&self, _replaced: usize) -> Result<(), MigrateError> {
        Ok(())
    }

    /// Called once every source has been replaced, before the new key is
    /// installed.
    fn after_commit(&self) -> Result<(), MigrateError> {
        Ok(())
    }

    /// Called between the two files of the vault's identity, when a rekey
    /// installs its new key.
    fn between_key_files(&self) -> Result<(), MigrateError> {
        Ok(())
    }
}

/// The hooks an ordinary run uses: none.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHooks;

impl MigrationHooks for NoHooks {}

/// What a conversion did.
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// Notes written in their new form.
    pub produced: usize,
    /// Notes skipped because a correct target was already there.
    pub already_done: usize,
    /// Notes verified against their sources.
    pub verified: usize,
    /// Source files removed at commit.
    pub deleted: usize,
    /// View directories removed, for a conversion to encrypted storage.
    pub views_removed: Vec<PathBuf>,
    /// Configured views whose directories were left in place, because they
    /// could not be removed safely.
    pub views_skipped: Vec<SkippedView>,
    /// Whether this run continued an interrupted conversion.
    pub resumed: bool,
}

/// Everything one conversion needs.
pub struct Migration<'a> {
    /// Which conversion this is.
    pub operation: Operation,
    /// Reads the notes as they are now.
    pub source: &'a dyn NoteCipher,
    /// Writes the notes in their new form.
    pub target: &'a dyn NoteCipher,
    /// Whether this run is finishing an interrupted conversion.
    pub resume: bool,
    /// The marker to write when starting fresh.
    pub marker: Marker,
    /// Where to stop, for crash-safety tests.
    pub hooks: &'a dyn MigrationHooks,
    /// The new vault identity a rekey installs once every note is replaced.
    pub handover: Option<KeyHandover>,
}

/// Run a whole-vault conversion.
pub fn run(
    session: &VaultSession,
    migration: &Migration<'_>,
) -> Result<MigrationReport, MigrateError> {
    let layout = session.layout();
    let all_notes = layout.all_notes();
    let marker_path = layout.migration_file();

    // Both ciphers must be able to read: the source to be converted, and the
    // target because every produced note is read back and compared before any
    // source is deleted. Verification is not optional, so a write-only target
    // is refused here rather than surfacing later as a verification failure
    // that looks like corruption.
    migration.source.readable()?;
    migration.target.readable()?;

    let existing = marker::read_at(&marker_path)?;
    match (&existing, migration.resume) {
        (None, true) => return Err(MigrateError::NothingToResume),
        (Some(found), false) => {
            return Err(MigrateError::AlreadyInProgress {
                operation: found.operation,
                resume: found.operation.resume_command(),
            });
        }
        // Finishing another operation's conversion would run it with the
        // wrong ciphers: a decrypt resumed over a rekey removes the key
        // files its notes still need.
        (Some(found), true) if found.operation != migration.operation => {
            return Err(MigrateError::AlreadyInProgress {
                operation: found.operation,
                resume: found.operation.resume_command(),
            });
        }
        _ => {}
    }

    // The marker goes down before the first target so that a crash between
    // here and the end is recognizable as an unfinished conversion rather than
    // a vault that mysteriously holds two copies of everything. A fresh run
    // first makes sure it will overwrite nothing; a resumed one has already
    // produced targets of its own, which the check would mistake for strays.
    if existing.is_none() {
        preflight(&all_notes, migration.operation, migration.source)?;
        marker::write_at(&marker_path, &migration.marker)?;
    }

    // A rekey interrupted between removing a source and renaming its
    // replacement holds that note under the temporary name only, where no scan
    // finds it.
    if migration.resume && migration.operation == Operation::Rekey {
        rekey::adopt_orphans(&all_notes, migration.target)?;
    }

    let scan = scan::scan_notes_dir(&all_notes, migration.source)?;
    let conversions = plan::conversions(&all_notes, migration.operation, &scan.notes);

    let mut report = MigrationReport {
        resumed: migration.resume,
        ..Default::default()
    };

    // -------------------------------------------------------------------------
    // Produce
    // -------------------------------------------------------------------------

    // Every target is made durable before any source is touched, and the
    // scope does that in one pass at its end: all targets flushed, then
    // renamed into place, then the directory flushed, rather than a flush
    // per note. Verify below reads the targets, so the scope ends first.
    let produced = fsutil::FlushScope::open();
    for conversion in &conversions {
        let plaintext = migration.source.read(&conversion.source)?;

        // On a resume, a target that already reads back correctly is left
        // alone, but still flushed: an earlier run may have been interrupted
        // before it got to. The check is a comparison and not an existence
        // test, because an existing file is not proof that it is right.
        if migration.resume && reads_back_as(migration.target, &conversion.target, &plaintext) {
            fsutil::make_durable(&conversion.target)?;
            report.already_done += 1;
            continue;
        }

        migration.target.write(&conversion.target, &plaintext)?;
        report.produced += 1;
    }
    produced.finish()?;

    migration.hooks.after_produce()?;

    // -------------------------------------------------------------------------
    // Verify
    // -------------------------------------------------------------------------

    for conversion in &conversions {
        let plaintext = migration.source.read(&conversion.source)?;
        if !reads_back_as(migration.target, &conversion.target, &plaintext) {
            return Err(MigrateError::VerificationFailed {
                note: conversion.source.clone(),
                target: conversion.target.clone(),
            });
        }
        report.verified += 1;
    }

    migration.hooks.after_verify()?;

    // The staged key is the only key the notes written from here on will have.
    // It is checked while every source is still on disk, so a staged pair that
    // changed since the marker was written costs nothing.
    if let Some(handover) = &migration.handover {
        handover.check_staged()?;
    }

    // -------------------------------------------------------------------------
    // Commit: the point of no return
    // -------------------------------------------------------------------------

    // The removals and renames share one directory flush, and it happens
    // before the marker goes: a removal that came back after a crash would
    // leave a vault with both forms of a note and no marker to say why.
    let removed = fsutil::FlushScope::open();
    for conversion in &conversions {
        // A rekey's produced file wears a temporary name, because its source
        // occupies the one it wants.
        if conversion.needs_rename() {
            fsutil::remove_file_durably(&conversion.source)?;
            fsutil::rename(&conversion.target, &conversion.final_target)?;
        } else {
            fsutil::remove_file_durably(&conversion.source)?;
        }
        report.deleted += 1;
        migration.hooks.after_commit_step(report.deleted)?;
    }
    removed.finish()?;

    migration.hooks.after_commit()?;

    if migration.operation == Operation::Encrypt {
        (report.views_removed, report.views_skipped) = remove_view_trees(session)?;
    }

    // Every note is written to the new key now, which makes the old key the
    // thing to retire. The marker stays until the new one is in place.
    if let Some(handover) = &migration.handover {
        let current = existing.as_ref().unwrap_or(&migration.marker);
        handover.install(
            &all_notes,
            migration.target,
            &marker_path,
            current,
            migration.hooks,
        )?;
    }

    marker::remove_at(&marker_path)?;
    Ok(report)
}

/// Refuse a conversion that would overwrite a note or leave one behind
/// (ADR 0067).
///
/// Two notes sharing a ULID would both be written to the one target their
/// ULID names, or the copy the scan skips would stay unconverted, in a rekey
/// encrypted to a key that is about to be replaced. A file already at a
/// target, which the source scan does not see as a note, would be replaced.
/// `vault encrypt` calls this before it writes the vault's keypair, which on
/// its own marks a vault as encrypted; `run` calls it again for every fresh
/// conversion.
pub fn preflight(
    all_notes: &Path,
    operation: Operation,
    source: &dyn NoteCipher,
) -> Result<(), MigrateError> {
    let scan = scan::scan_notes_dir(all_notes, source)?;
    if !scan.duplicates.is_empty() {
        let copies = scan.duplicates.into_iter().map(|d| d.note.path).collect();
        return Err(MigrateError::DuplicateIds(copies));
    }

    let conversions = plan::conversions(all_notes, operation, &scan.notes);
    let mut occupied: Vec<PathBuf> = conversions
        .into_iter()
        .flat_map(|c| {
            [&c.target, &c.final_target]
                .into_iter()
                .filter(|target| holds_another_file(target, &c.source))
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect();
    occupied.sort();
    occupied.dedup();
    if occupied.is_empty() {
        Ok(())
    } else {
        Err(MigrateError::TargetsExist {
            operation,
            paths: occupied,
        })
    }
}

/// Whether something other than `source` itself sits at `target`.
///
/// A rekey's final target is its source's own name, and on a case-insensitive
/// filesystem a target can name the source under another spelling; neither
/// is in the way.
fn holds_another_file(target: &Path, source: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Ok(found) = std::fs::symlink_metadata(target) else {
        return false;
    };
    !std::fs::symlink_metadata(source)
        .is_ok_and(|own| own.dev() == found.dev() && own.ino() == found.ino())
}

/// Whether `path` reads back through `cipher` as exactly `expected`.
///
/// Any failure — absent, unreadable, not decryptable — answers "no", which is
/// the same answer a mismatch gives and leads to the same action: produce it
/// again.
fn reads_back_as(cipher: &dyn NoteCipher, path: &Path, expected: &str) -> bool {
    cipher.read(path).is_ok_and(|actual| actual == expected)
}

/// Remove the materialized view trees, which an encrypted vault cannot have.
///
/// Done at commit rather than up front so an interrupted conversion leaves the
/// vault's views intact and usable. A view whose tree cannot be removed safely
/// is skipped and reported rather than failing the run: this runs after the
/// sources are gone, and failing here would leave the conversion marker
/// behind.
fn remove_view_trees(
    session: &VaultSession,
) -> Result<(Vec<PathBuf>, Vec<SkippedView>), MigrateError> {
    let (dirs, skipped) = removable_view_dirs(session)?;

    let mut removed = Vec::new();
    for dir in dirs {
        if dir.is_dir() {
            fsutil::remove_dir_all(&dir)?;
            removed.push(dir);
        }
    }
    removed.sort();
    Ok((removed, skipped))
}

/// The configured views whose trees `vault encrypt` could not remove safely.
///
/// A conversion leaves such a tree in the vault, where its link names spell
/// out note titles in plaintext, so `vault encrypt` asks for this before it
/// writes anything and refuses while the list is not empty.
pub fn views_blocking_encrypt(vault: &Vault) -> Result<Vec<SkippedView>, MigrateError> {
    let (_, skipped) = removable_view_dirs(vault)?;
    Ok(skipped)
}

/// The directories of the configured views that can be removed safely, and
/// the views that cannot: those with an invalid name, and those whose
/// directory sits behind a symlink or cannot be inspected.
fn removable_view_dirs(vault: &Vault) -> Result<(Vec<PathBuf>, Vec<SkippedView>), MigrateError> {
    let config = crate::config::PerVaultConfig::load(&vault.layout().config_file())?;
    let (views, mut skipped) = reconcile::configured_views(&config);

    let mut dirs = Vec::new();
    for view in &views {
        match view::contained_dir(vault.layout(), &view.name) {
            Ok(dir) => dirs.push(dir),
            Err(error) => skipped.push(SkippedView::unsafe_dir(&view.name, error)),
        }
    }
    Ok((dirs, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cipher::{AgeCipher, PlaintextCipher};
    use crate::crypto::age_io;
    use crate::test_support::encrypted::encrypted_vault;
    use crate::vault::Vault;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// Stops a conversion at a phase boundary, the way a crash would.
    struct FailAfter(Phase);

    impl MigrationHooks for FailAfter {
        fn after_produce(&self) -> Result<(), MigrateError> {
            match self.0 {
                Phase::Produce => Err(MigrateError::Interrupted(Phase::Produce)),
                _ => Ok(()),
            }
        }

        fn after_verify(&self) -> Result<(), MigrateError> {
            match self.0 {
                Phase::Verify => Err(MigrateError::Interrupted(Phase::Verify)),
                _ => Ok(()),
            }
        }
    }

    /// A plaintext vault holding two notes, ready to be encrypted.
    fn plaintext_vault() -> (tempfile::TempDir, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        let session = plaintext_vault_at(dir.path());
        (dir, session)
    }

    /// The same vault at `<base>/vault`, so a view name that climbs out of
    /// the vault still lands inside the test's own temp dir.
    fn nested_plaintext_vault() -> (tempfile::TempDir, PathBuf, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().to_path_buf();
        let session = plaintext_vault_at(&base.join("vault"));
        (dir, base, session)
    }

    fn plaintext_vault_at(root: &Path) -> VaultSession {
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        std::fs::write(
            root.join("all-notes").join(format!("{ULID_A}-alpha.md")),
            "---\ntitle: Alpha\n---\nfirst body\n",
        )
        .expect("write note");
        std::fs::write(
            root.join("all-notes").join(format!("{ULID_B}-beta.md")),
            "---\ntitle: Beta\n---\nsecond body\n",
        )
        .expect("write note");
        VaultSession::plaintext(Vault::new(root))
    }

    /// Write a config holding the given `(name, field)` views verbatim.
    fn configure_views(session: &VaultSession, views: &[(&str, &str)]) {
        let mut config = crate::config::PerVaultConfig::default();
        for (name, field) in views {
            config.add(crate::config::ViewConfig {
                name: (*name).into(),
                field: (*field).into(),
            });
        }
        std::fs::write(
            session.layout().config_file(),
            config.to_toml().expect("toml"),
        )
        .expect("write config");
    }

    fn encrypt(session: &VaultSession) -> MigrationReport {
        let (recipient, target) = fresh_target();
        run(
            session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("encrypt")
    }

    /// A keypair plus a cipher that can both write and read it back.
    ///
    /// A conversion verifies by reading its targets, so the target cipher
    /// always carries the identity — which `vault encrypt` has anyway, having
    /// just generated it.
    fn fresh_target() -> (crate::crypto::Recipient, AgeCipher) {
        let (identity, recipient) = age_io::generate_keypair();
        (recipient.clone(), AgeCipher::new(recipient, Some(identity)))
    }

    fn marker_for(operation: Operation, recipient: Option<String>) -> Marker {
        Marker::new(operation, recipient, "2026-08-06T12:00:00Z".into())
    }

    /// Every filename directly inside `all-notes/`, sorted.
    fn listing(session: &VaultSession) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(session.layout().all_notes())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Every note's plaintext, keyed by filename, for comparing two vaults.
    fn contents(session: &VaultSession, cipher: &dyn NoteCipher) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = std::fs::read_dir(session.layout().all_notes())
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(cipher.extension()))
            .map(|p| {
                let name = p.file_name().expect("name").to_string_lossy().into_owned();
                (name, cipher.read(&p).expect("read"))
            })
            .collect();
        out.sort();
        out
    }

    // -------------------------------------------------------------------------
    // Conversions that would overwrite a note (ADR 0067)
    // -------------------------------------------------------------------------

    /// Start a fresh conversion of `operation`.
    fn start(
        session: &VaultSession,
        operation: Operation,
        source: &dyn NoteCipher,
        target: &dyn NoteCipher,
    ) -> Result<MigrationReport, MigrateError> {
        run(
            session,
            &Migration {
                operation,
                source,
                target,
                resume: false,
                marker: marker_for(operation, None),
                hooks: &NoHooks,
                handover: None,
            },
        )
    }

    #[test]
    fn encrypt_refuses_while_two_notes_share_a_ulid() {
        let (_guard, session) = plaintext_vault();
        std::fs::write(
            session
                .layout()
                .all_notes()
                .join(format!("{ULID_A}-alpha-copy.md")),
            "---\ntitle: Alpha\n---\ncopied body\n",
        )
        .expect("write copy");
        let before = listing(&session);
        let (_, target) = fresh_target();

        let result = start(&session, Operation::Encrypt, &PlaintextCipher, &target);

        let err = result.expect_err("a copy would be left behind or overwritten");
        assert!(
            err.to_string().contains(&format!("{ULID_A}-alpha-copy.md")),
            "{err}"
        );
        assert_eq!(listing(&session), before, "nothing was converted");
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn encrypt_refuses_when_a_target_file_exists() {
        // A stray `.age` file is a resource in a plaintext vault, but the
        // conversion would write a note over it.
        let (_guard, session) = plaintext_vault();
        let stray = session.layout().all_notes().join(format!("{ULID_A}.age"));
        std::fs::write(&stray, "stray bytes").expect("write stray");
        let (_, target) = fresh_target();

        let result = start(&session, Operation::Encrypt, &PlaintextCipher, &target);

        let err = result.expect_err("the stray file would be overwritten");
        assert!(err.to_string().contains(&format!("{ULID_A}.age")), "{err}");
        assert_eq!(std::fs::read(&stray).expect("read stray"), b"stray bytes");
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn decrypt_refuses_when_a_target_file_exists() {
        // A plaintext note dropped next to the encrypted note it would be
        // decrypted to: the encrypted scan sees only the `.age` file.
        let fixture = encrypted_vault();
        let session = &fixture.session;
        crate::test_support::encrypted::write_encrypted_note(
            session,
            ULID_A,
            "---\ntitle: Plan\n---\nencrypted\n",
        );
        let stray = session
            .layout()
            .all_notes()
            .join(format!("{ULID_A}-plan.md"));
        std::fs::write(&stray, "---\ntitle: Plan\n---\nstray\n").expect("write stray");

        let result = start(
            session,
            Operation::Decrypt,
            session.cipher(),
            &PlaintextCipher,
        );

        let err = result.expect_err("the stray note would be overwritten");
        assert!(err.to_string().contains("ntropy reconcile"), "{err}");
        assert_eq!(
            std::fs::read_to_string(&stray).expect("read stray"),
            "---\ntitle: Plan\n---\nstray\n"
        );
        assert!(!session.layout().migration_file().exists());
    }

    // -------------------------------------------------------------------------
    // The happy path
    // -------------------------------------------------------------------------

    #[test]
    fn encrypt_converts_every_note_and_removes_the_marker() {
        let (_guard, session) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let target = AgeCipher::new(recipient.clone(), Some(identity.clone()));

        let report = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("encrypt");

        assert_eq!(report.produced, 2);
        assert_eq!(report.verified, 2);
        assert_eq!(report.deleted, 2);
        assert!(!session.layout().migration_file().exists());
        assert_eq!(
            listing(&session),
            [format!("{ULID_A}.age"), format!("{ULID_B}.age")]
        );

        // The notes really are the notes.
        let reader = AgeCipher::new(identity.to_public(), Some(identity));
        let by_name = contents(&session, &reader);
        assert_eq!(by_name[0].1, "---\ntitle: Alpha\n---\nfirst body\n");
        assert_eq!(by_name[1].1, "---\ntitle: Beta\n---\nsecond body\n");
    }

    #[test]
    fn decrypt_restores_the_notes_content_for_content() {
        let (_guard, session) = plaintext_vault();
        let before = contents(&session, &PlaintextCipher);

        let (identity, recipient) = age_io::generate_keypair();
        let encrypted = AgeCipher::new(recipient.clone(), Some(identity));
        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &encrypted,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("encrypt");

        run(
            &session,
            &Migration {
                operation: Operation::Decrypt,
                source: &encrypted,
                target: &PlaintextCipher,
                resume: false,
                marker: marker_for(Operation::Decrypt, None),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("decrypt");

        assert_eq!(contents(&session, &PlaintextCipher), before);
    }

    #[test]
    fn encrypting_an_empty_vault_succeeds() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));
        let (recipient, target) = fresh_target();

        let report = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("encrypt");
        assert_eq!(report.produced, 0);
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn rekey_re_encrypts_every_note_to_the_new_key() {
        let fixture = encrypted_vault();
        let session = &fixture.session;
        crate::test_support::encrypted::write_encrypted_note(
            session,
            ULID_A,
            "---\ntitle: Alpha\n---\nbody\n",
        );

        let (new_identity, new_recipient) = age_io::generate_keypair();
        let report = run(
            session,
            &Migration {
                operation: Operation::Rekey,
                source: session.cipher(),
                target: &AgeCipher::new(new_recipient.clone(), Some(new_identity.clone())),
                resume: false,
                marker: marker_for(Operation::Rekey, Some(new_recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("rekey");

        assert_eq!(report.produced, 1);
        // The name is unchanged; only what is inside it changed.
        assert_eq!(listing(session), [format!("{ULID_A}.age")]);

        let path = session.layout().all_notes().join(format!("{ULID_A}.age"));
        let new_reader = AgeCipher::new(new_identity.to_public(), Some(new_identity));
        assert_eq!(
            new_reader.read(&path).expect("read with the new key"),
            "---\ntitle: Alpha\n---\nbody\n"
        );
        assert!(
            session.cipher().read(&path).is_err(),
            "the old key must no longer open it"
        );
    }

    #[test]
    fn encrypt_removes_the_view_trees_at_commit() {
        let (_guard, session) = plaintext_vault();
        let mut config = crate::config::PerVaultConfig::default();
        config.add(crate::config::ViewConfig {
            name: "by-tag".into(),
            field: "tags".into(),
        });
        std::fs::write(
            session.layout().config_file(),
            config.to_toml().expect("toml"),
        )
        .expect("write config");
        std::fs::create_dir_all(session.root().join("by-tag").join("work")).expect("view tree");

        let (recipient, target) = fresh_target();
        let report = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("encrypt");

        assert_eq!(report.views_removed, [session.root().join("by-tag")]);
        assert!(!session.root().join("by-tag").exists());
        assert!(report.views_skipped.is_empty());
    }

    #[test]
    fn encrypt_never_removes_a_tree_outside_the_vault() {
        let (guard, base, session) = nested_plaintext_vault();
        let victim = base.join("decoy").join("sub").join("keep.txt");
        std::fs::create_dir_all(victim.parent().expect("a victim has a parent")).expect("decoy");
        std::fs::write(&victim, "keep me").expect("victim");
        configure_views(&session, &[("../decoy", "tags")]);

        let report = encrypt(&session);

        assert!(victim.starts_with(guard.path()));
        assert!(victim.exists(), "the decoy tree was removed");
        assert!(report.views_removed.is_empty());
        let skipped: Vec<&str> = report
            .views_skipped
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(skipped, ["../decoy"]);
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn encrypt_leaves_a_symlinked_view_root_and_reports_it() {
        let (guard, base, session) = nested_plaintext_vault();
        let decoy = base.join("decoy");
        std::fs::create_dir_all(&decoy).expect("decoy");
        std::fs::write(decoy.join("keep.txt"), "keep").expect("decoy file");
        let link = session.root().join("by-tag");
        std::os::unix::fs::symlink(&decoy, &link).expect("plant symlink");
        configure_views(&session, &[("by-tag", "tags")]);

        let report = encrypt(&session);

        assert!(decoy.join("keep.txt").exists());
        assert!(decoy.starts_with(guard.path()));
        // The link is the user's to remove: ntropy reports it rather than
        // touching a path that leads out of the vault.
        assert!(link.symlink_metadata().is_ok(), "the link was removed");
        assert!(report.views_removed.is_empty());
        let skipped: Vec<&str> = report
            .views_skipped
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(skipped, ["by-tag"]);
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn encrypt_never_removes_a_tree_behind_a_symlinked_segment() {
        let (guard, base, session) = nested_plaintext_vault();
        let victim = base.join("decoy").join("work").join("keep.txt");
        std::fs::create_dir_all(victim.parent().expect("a victim has a parent")).expect("decoy");
        std::fs::write(&victim, "keep me").expect("victim");
        std::os::unix::fs::symlink(base.join("decoy"), session.root().join("area"))
            .expect("plant symlink");
        configure_views(&session, &[("area/work", "tags")]);

        let report = encrypt(&session);

        assert!(victim.starts_with(guard.path()));
        assert!(victim.exists(), "the tree behind the symlink was removed");
        let skipped: Vec<&str> = report
            .views_skipped
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(skipped, ["area/work"]);
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn a_regular_file_in_a_view_path_does_not_stop_the_commit() {
        let (_guard, session) = plaintext_vault();
        std::fs::write(session.root().join("area"), "a file").expect("file");
        configure_views(&session, &[("area/work", "tags")]);

        let report = encrypt(&session);

        assert!(report.views_removed.is_empty());
        assert!(report.views_skipped.is_empty());
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn a_view_path_that_cannot_be_inspected_does_not_stop_the_commit() {
        let (_guard, session) = plaintext_vault();
        let view = crate::test_support::uninspectable_view(session.root());
        configure_views(&session, &[(view.as_str(), "tags")]);

        let report = encrypt(&session);

        assert_eq!(report.views_skipped.len(), 1);
        assert!(matches!(
            report.views_skipped[0].reason,
            crate::view::SkipReason::Unreadable { .. }
        ));
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn unsafe_view_directories_block_encrypt() {
        let (_guard, base, session) = nested_plaintext_vault();
        std::fs::create_dir_all(base.join("decoy")).expect("decoy");
        std::os::unix::fs::symlink(base.join("decoy"), session.root().join("by-tag"))
            .expect("plant symlink");
        let long = crate::test_support::uninspectable_view(session.root());
        configure_views(&session, &[("by-tag", "tags"), (long.as_str(), "tags")]);

        let blocking = views_blocking_encrypt(&session).expect("check");

        assert_eq!(blocking.len(), 2);
        assert!(matches!(
            blocking[0].reason,
            crate::view::SkipReason::Symlinked(_)
        ));
        assert!(matches!(
            blocking[1].reason,
            crate::view::SkipReason::Unreadable { .. }
        ));
    }

    #[test]
    fn views_with_invalid_names_block_encrypt() {
        let (_guard, session) = plaintext_vault();
        configure_views(&session, &[("by-tag", "tags"), ("By_Tag", "tags")]);
        let blocking = views_blocking_encrypt(&session).expect("check");
        let names: Vec<&str> = blocking.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["By_Tag"]);
    }

    #[test]
    fn valid_views_do_not_block_encrypt() {
        let (_guard, session) = plaintext_vault();
        configure_views(&session, &[("by-tag", "tags")]);
        assert!(views_blocking_encrypt(&session).expect("check").is_empty());
    }

    // -------------------------------------------------------------------------
    // Interruption and resume
    // -------------------------------------------------------------------------

    #[test]
    fn a_crash_after_produce_leaves_both_forms_and_the_marker() {
        let (_guard, session) = plaintext_vault();
        let (recipient, target) = fresh_target();

        let err = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &FailAfter(Phase::Produce),
                handover: None,
            },
        )
        .expect_err("interrupted");
        assert!(matches!(err, MigrateError::Interrupted(Phase::Produce)));

        // Every note exists twice, and the marker says a conversion is running.
        assert_eq!(
            listing(&session),
            [
                format!("{ULID_A}-alpha.md"),
                format!("{ULID_A}.age"),
                format!("{ULID_B}-beta.md"),
                format!("{ULID_B}.age"),
            ]
        );
        assert!(session.layout().migration_file().is_file());
    }

    #[test]
    fn a_crash_after_verify_still_deletes_nothing() {
        // The last window before the point of no return.
        let (_guard, session) = plaintext_vault();
        let (recipient, target) = fresh_target();

        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &FailAfter(Phase::Verify),
                handover: None,
            },
        )
        .expect_err("interrupted");

        assert!(
            session
                .layout()
                .all_notes()
                .join(format!("{ULID_A}-alpha.md"))
                .exists(),
            "no source may be removed before commit"
        );
        assert!(session.layout().migration_file().is_file());
    }

    #[test]
    fn resuming_converges_on_the_same_result_as_an_uninterrupted_run() {
        let (_guard_a, interrupted) = plaintext_vault();
        let (_guard_b, straight) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let reader = AgeCipher::new(identity.to_public(), Some(identity));

        let migration = |resume| Migration {
            operation: Operation::Encrypt,
            source: &PlaintextCipher,
            target: &reader,
            resume,
            marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
            hooks: &NoHooks,
            handover: None,
        };

        // One vault is interrupted and then resumed.
        run(
            &interrupted,
            &Migration {
                hooks: &FailAfter(Phase::Produce),
                handover: None,
                ..migration(false)
            },
        )
        .expect_err("interrupted");
        let resumed = run(&interrupted, &migration(true)).expect("resume");
        assert!(resumed.resumed);

        // The other runs straight through.
        run(&straight, &migration(false)).expect("uninterrupted");

        assert_eq!(listing(&interrupted), listing(&straight));
        assert_eq!(
            contents(&interrupted, &reader),
            contents(&straight, &reader)
        );
        assert!(!interrupted.layout().migration_file().exists());
    }

    #[test]
    fn resuming_re_produces_a_corrupted_target() {
        // The test that fails against an existence check. A target that was
        // renamed into place but whose contents never reached the disk exists
        // and is wrong; only reading it back catches that.
        let (_guard, session) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let cipher = AgeCipher::new(identity.to_public(), Some(identity));

        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &cipher,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &FailAfter(Phase::Produce),
                handover: None,
            },
        )
        .expect_err("interrupted");

        // Truncate one produced target, as a power loss could.
        let damaged = session.layout().all_notes().join(format!("{ULID_A}.age"));
        std::fs::write(&damaged, b"").expect("truncate");

        let report = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &cipher,
                resume: true,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("resume");

        assert_eq!(report.produced, 1, "the damaged target must be rewritten");
        assert_eq!(report.already_done, 1, "the intact one must be left alone");
        assert_eq!(
            cipher.read(&damaged).expect("read"),
            "---\ntitle: Alpha\n---\nfirst body\n"
        );
    }

    #[test]
    fn resuming_twice_is_harmless() {
        let (_guard, session) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let cipher = AgeCipher::new(identity.to_public(), Some(identity));

        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &cipher,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &FailAfter(Phase::Produce),
                handover: None,
            },
        )
        .expect_err("interrupted");
        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &cipher,
                resume: true,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("first resume");

        // With the conversion finished the marker is gone, so a second resume
        // has nothing to finish and says so rather than starting over.
        let err = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &cipher,
                resume: true,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect_err("nothing left");
        assert!(matches!(err, MigrateError::NothingToResume));
    }

    #[test]
    fn resuming_with_nothing_in_progress_is_an_error() {
        let (_guard, session) = plaintext_vault();
        let (recipient, target) = fresh_target();
        let err = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: true,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect_err("nothing to resume");
        assert!(matches!(err, MigrateError::NothingToResume));
    }

    #[test]
    fn starting_a_second_conversion_is_refused() {
        let (_guard, session) = plaintext_vault();
        let (recipient, target) = fresh_target();
        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &FailAfter(Phase::Produce),
                handover: None,
            },
        )
        .expect_err("interrupted");

        let err = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect_err("already running");
        match err {
            MigrateError::AlreadyInProgress { operation, resume } => {
                assert_eq!(operation, Operation::Encrypt);
                assert!(resume.contains("--resume"), "{resume}");
            }
            other => panic!("expected AlreadyInProgress, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // Verification
    // -------------------------------------------------------------------------

    #[test]
    fn a_verification_failure_deletes_nothing() {
        // A cipher that writes something other than what it was given, which is
        // what a silently corrupting storage layer would look like.
        #[derive(Debug)]
        struct Liar(AgeCipher);

        impl NoteCipher for Liar {
            fn extension(&self) -> &'static str {
                self.0.extension()
            }
            fn readable(&self) -> Result<(), CipherError> {
                self.0.readable()
            }
            fn read(&self, path: &Path) -> Result<String, CipherError> {
                self.0.read(path)
            }
            fn write(&self, path: &Path, _content: &str) -> Result<(), CipherError> {
                self.0.write(path, "not what you asked for")
            }
        }

        let (_guard, session) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let liar = Liar(AgeCipher::new(identity.to_public(), Some(identity)));

        let err = run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &liar,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect_err("verification fails");
        assert!(matches!(err, MigrateError::VerificationFailed { .. }));

        assert!(
            session
                .layout()
                .all_notes()
                .join(format!("{ULID_A}-alpha.md"))
                .exists(),
            "the sources must survive a failed verification"
        );
        assert!(session.layout().migration_file().is_file());
    }

    #[test]
    fn the_marker_survives_so_the_next_command_refuses() {
        let (_guard, session) = plaintext_vault();
        let (recipient, target) = fresh_target();
        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &target,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &FailAfter(Phase::Produce),
                handover: None,
            },
        )
        .expect_err("interrupted");

        let found = marker::read_at(&session.layout().migration_file())
            .expect("read")
            .expect("present");
        assert_eq!(found.operation, Operation::Encrypt);
        assert_eq!(
            found.recipient.as_deref(),
            Some(recipient.as_str().as_str())
        );
    }

    #[test]
    fn a_rekey_crash_leaves_the_sources_readable_by_the_old_key() {
        // Resume has to be able to read what it has not converted yet, so the
        // old key must still be the one on the sources.
        let fixture = encrypted_vault();
        let session = &fixture.session;
        crate::test_support::encrypted::write_encrypted_note(
            session,
            ULID_A,
            "---\ntitle: Alpha\n---\nbody\n",
        );

        let (new_identity, new_recipient) = age_io::generate_keypair();
        run(
            session,
            &Migration {
                operation: Operation::Rekey,
                source: session.cipher(),
                target: &AgeCipher::new(new_recipient.clone(), Some(new_identity)),
                resume: false,
                marker: marker_for(Operation::Rekey, Some(new_recipient.as_str())),
                hooks: &FailAfter(Phase::Produce),
                handover: None,
            },
        )
        .expect_err("interrupted");

        let source = session.layout().all_notes().join(format!("{ULID_A}.age"));
        assert_eq!(
            session
                .cipher()
                .read(&source)
                .expect("read with the old key"),
            "---\ntitle: Alpha\n---\nbody\n"
        );
        assert!(
            session
                .layout()
                .all_notes()
                .join(format!("{ULID_A}.age{}", plan::REKEY_SUFFIX))
                .is_file(),
            "the produced file waits under its temporary name"
        );
    }

    #[test]
    fn a_resumed_rekey_converges() {
        let fixture = encrypted_vault();
        let session = &fixture.session;
        crate::test_support::encrypted::write_encrypted_note(
            session,
            ULID_A,
            "---\ntitle: Alpha\n---\nbody\n",
        );

        let (new_identity, new_recipient) = age_io::generate_keypair();
        let target = AgeCipher::new(new_recipient.clone(), Some(new_identity.clone()));
        let migration = |resume, hooks: &'static dyn MigrationHooks| Migration {
            operation: Operation::Rekey,
            source: session.cipher(),
            target: &target,
            resume,
            marker: marker_for(Operation::Rekey, Some(new_recipient.as_str())),
            hooks,
            handover: None,
        };

        run(session, &migration(false, &FailAfter(Phase::Verify))).expect_err("interrupted");
        run(session, &migration(true, &NoHooks)).expect("resume");

        assert_eq!(listing(session), [format!("{ULID_A}.age")]);
        let path = session.layout().all_notes().join(format!("{ULID_A}.age"));
        let reader = AgeCipher::new(new_identity.to_public(), Some(new_identity));
        assert_eq!(
            reader.read(&path).expect("read"),
            "---\ntitle: Alpha\n---\nbody\n"
        );
        assert!(!session.layout().migration_file().exists());
    }

    #[test]
    fn a_resume_of_one_operation_refuses_the_marker_of_another() {
        // `vault decrypt` removes the key files once it finishes, which a
        // rekey in progress still needs for its notes.
        let fixture = encrypted_vault();
        let session = &fixture.session;
        let rekeying = marker_for(Operation::Rekey, Some("age1fake".into()));
        marker::write_at(&session.layout().migration_file(), &rekeying).expect("marker");

        let error = run(
            session,
            &Migration {
                operation: Operation::Decrypt,
                source: session.cipher(),
                target: &PlaintextCipher,
                resume: true,
                marker: marker_for(Operation::Decrypt, None),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect_err("refused");

        assert!(matches!(
            error,
            MigrateError::AlreadyInProgress {
                operation: Operation::Rekey,
                ..
            }
        ));
        assert_eq!(
            marker::read_at(&session.layout().migration_file()).expect("read"),
            Some(rekeying)
        );
    }
    // -------------------------------------------------------------------------
    // Flush order
    // -------------------------------------------------------------------------

    fn positions(events: &[String], prefix: &str) -> Vec<usize> {
        events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.starts_with(prefix))
            .map(|(index, _)| index)
            .collect()
    }

    /// Assert the order every conversion keeps: all produced files flushed
    /// before their first rename into place, the directory flushed before the
    /// first source goes, and the directory flushed again after the last
    /// source is gone, before the marker is removed.
    fn assert_conversion_flush_order(events: &[String]) {
        let flushed_temps: Vec<usize> = positions(events, "flush all-notes/")
            .into_iter()
            .filter(|i| events[*i].ends_with(".tmp"))
            .collect();
        let temp_renames: Vec<usize> = positions(events, "rename all-notes/")
            .into_iter()
            .filter(|i| events[*i].contains(".tmp -> "))
            .collect();
        let directory_flushes = positions(events, "flush-dir all-notes");
        let removals = positions(events, "remove all-notes/");
        let marker_removal = positions(events, "remove .ntropy/migration.toml");

        assert!(!flushed_temps.is_empty(), "events were: {events:?}");
        assert!(
            flushed_temps.iter().max() < temp_renames.iter().min(),
            "a file was renamed before every file was flushed: {events:?}"
        );
        assert_eq!(directory_flushes.len(), 2, "events were: {events:?}");
        assert!(
            temp_renames.iter().max() < directory_flushes.first()
                && directory_flushes.first() < removals.iter().min(),
            "the directory is flushed once all targets are in place and before any source goes: {events:?}"
        );
        assert!(
            removals.iter().max() < directory_flushes.last()
                && directory_flushes.last() < marker_removal.first(),
            "the removals are flushed before the marker goes: {events:?}"
        );
    }

    #[test]
    fn encrypt_flushes_every_target_before_any_source_goes_and_the_removals_before_the_marker() {
        let (guard, session) = plaintext_vault();

        let (_, events) = crate::fsutil::trace::capture(guard.path(), || encrypt(&session));

        assert_conversion_flush_order(&events);
    }

    #[test]
    fn decrypt_flushes_every_target_before_any_source_goes_and_the_removals_before_the_marker() {
        let (guard, session) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let encrypted = AgeCipher::new(recipient.clone(), Some(identity));
        run(
            &session,
            &Migration {
                operation: Operation::Encrypt,
                source: &PlaintextCipher,
                target: &encrypted,
                resume: false,
                marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
                hooks: &NoHooks,
                handover: None,
            },
        )
        .expect("encrypt");

        let (_, events) = crate::fsutil::trace::capture(guard.path(), || {
            run(
                &session,
                &Migration {
                    operation: Operation::Decrypt,
                    source: &encrypted,
                    target: &PlaintextCipher,
                    resume: false,
                    marker: marker_for(Operation::Decrypt, None),
                    hooks: &NoHooks,
                    handover: None,
                },
            )
            .expect("decrypt")
        });

        assert_conversion_flush_order(&events);
    }

    #[test]
    fn rekey_renames_and_removals_share_one_directory_flush_before_the_marker_goes() {
        let fixture = encrypted_vault();
        let session = &fixture.session;
        for ulid in [ULID_A, ULID_B] {
            crate::test_support::encrypted::write_encrypted_note(
                session,
                ulid,
                "---\ntitle: Note\n---\nbody\n",
            );
        }
        let (new_identity, new_recipient) = age_io::generate_keypair();
        let target = AgeCipher::new(new_recipient.clone(), Some(new_identity));

        let (_, events) = crate::fsutil::trace::capture(session.root(), || {
            run(
                session,
                &Migration {
                    operation: Operation::Rekey,
                    source: session.cipher(),
                    target: &target,
                    resume: false,
                    marker: marker_for(Operation::Rekey, Some(new_recipient.as_str())),
                    hooks: &NoHooks,
                    handover: None,
                },
            )
            .expect("rekey")
        });

        let directory_flushes = positions(&events, "flush-dir all-notes");
        assert_eq!(directory_flushes.len(), 2, "events were: {events:?}");
        let last_rename_into_place = positions(&events, "rename all-notes/")
            .into_iter()
            .filter(|i| events[*i].ends_with(".age"))
            .max()
            .expect("rename");
        let marker_removal = positions(&events, "remove .ntropy/migration.toml");
        assert!(
            last_rename_into_place < directory_flushes[1]
                && Some(&directory_flushes[1]) < marker_removal.first(),
            "events were: {events:?}"
        );
    }

    #[test]
    fn a_resumed_conversion_flushes_the_targets_it_skips() {
        let (guard, session) = plaintext_vault();
        let (identity, recipient) = age_io::generate_keypair();
        let cipher = AgeCipher::new(identity.to_public(), Some(identity));
        let migration = |resume, hooks: &'static dyn MigrationHooks| Migration {
            operation: Operation::Encrypt,
            source: &PlaintextCipher,
            target: &cipher,
            resume,
            marker: marker_for(Operation::Encrypt, Some(recipient.as_str())),
            hooks,
            handover: None,
        };
        run(&session, &migration(false, &FailAfter(Phase::Produce))).expect_err("interrupted");

        let (report, events) = crate::fsutil::trace::capture(guard.path(), || {
            run(&session, &migration(true, &NoHooks)).expect("resume")
        });

        assert_eq!(report.already_done, 2);
        let first_removal = positions(&events, "remove all-notes/")[0];
        for ulid in [ULID_A, ULID_B] {
            let flush = events
                .iter()
                .position(|e| *e == format!("flush all-notes/{ulid}.age"))
                .unwrap_or_else(|| panic!("{ulid}.age was not flushed: {events:?}"));
            assert!(flush < first_removal, "events were: {events:?}");
        }
    }
}
