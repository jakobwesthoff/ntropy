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
//!    complete copy of itself in at least one form.
//! 2. **Verify.** Every produced file is read back and compared against its
//!    source's plaintext. A mismatch aborts while both forms are still on disk.
//! 3. **Commit.** The produced files are flushed to stable storage, then the
//!    sources are deleted and the marker removed. This is the point of no
//!    return, and nothing before it loses information.
//!
//! `--resume` re-verifies by reading and comparing, never by checking that a
//! target exists. A file that was renamed into place but whose contents never
//! reached the disk exists and is wrong, which is precisely the case the
//! existence check would wave through.

pub mod marker;
pub mod plan;

use std::path::{Path, PathBuf};

pub use marker::{Marker, Operation};
pub use plan::Conversion;

use crate::cipher::{CipherError, NoteCipher};
use crate::fsutil::{self, FsError};
use crate::scan::{self, ScanError};
use crate::session::VaultSession;

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
}

/// The hooks an ordinary run uses: none.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHooks;

impl MigrationHooks for NoHooks {}

/// What a conversion did.
#[derive(Debug, Default, PartialEq, Eq)]
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
        _ => {}
    }

    // The marker goes down before the first target so that a crash between
    // here and the end is recognizable as an unfinished conversion rather than
    // a vault that mysteriously holds two copies of everything.
    if existing.is_none() {
        marker::write_at(&marker_path, &migration.marker)?;
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

    for conversion in &conversions {
        let plaintext = migration.source.read(&conversion.source)?;

        // On a resume, a target that already reads back correctly is left
        // alone. The check is a comparison and not an existence test: a file
        // that was renamed into place but whose contents never reached the
        // disk exists and is wrong.
        if migration.resume && reads_back_as(migration.target, &conversion.target, &plaintext) {
            report.already_done += 1;
            continue;
        }

        migration.target.write(&conversion.target, &plaintext)?;
        report.produced += 1;
    }

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

    // -------------------------------------------------------------------------
    // Commit: the point of no return
    // -------------------------------------------------------------------------

    // One barrier over everything produced, rather than a flush per file. The
    // guarantee is identical — every target is durable before any source is
    // removed — and the I/O scheduler gets to coalesce the work instead of
    // being made to wait once per note.
    for conversion in &conversions {
        fsutil::sync_file(&conversion.target)?;
    }
    fsutil::sync_dir(&all_notes)?;

    for conversion in &conversions {
        // A rekey's produced file wears a temporary name, because its source
        // occupies the one it wants.
        if conversion.needs_rename() {
            fsutil::remove_file(&conversion.source)?;
            fsutil::rename(&conversion.target, &conversion.final_target)?;
        } else {
            fsutil::remove_file(&conversion.source)?;
        }
        report.deleted += 1;
    }

    if migration.operation == Operation::Encrypt {
        report.views_removed = remove_view_trees(session)?;
    }

    marker::remove_at(&marker_path)?;
    Ok(report)
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
/// vault's views intact and usable.
fn remove_view_trees(session: &VaultSession) -> Result<Vec<PathBuf>, MigrateError> {
    let config = crate::config::PerVaultConfig::load(&session.layout().config_file())?;

    let mut removed = Vec::new();
    for view in &config.views {
        let dir = session.layout().view_dir(&view.name);
        if dir.is_dir() {
            fsutil::remove_dir_all(&dir)?;
            removed.push(dir);
        }
    }
    removed.sort();
    Ok(removed)
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
        let root = dir.path();
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
        let session = VaultSession::plaintext(Vault::new(root));
        (dir, session)
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
        std::fs::create_dir_all(session.layout().view_dir("by-tag").join("work"))
            .expect("view tree");

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
            },
        )
        .expect("encrypt");

        assert_eq!(report.views_removed, [session.layout().view_dir("by-tag")]);
        assert!(!session.layout().view_dir("by-tag").exists());
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
        };

        // One vault is interrupted and then resumed.
        run(
            &interrupted,
            &Migration {
                hooks: &FailAfter(Phase::Produce),
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
}
