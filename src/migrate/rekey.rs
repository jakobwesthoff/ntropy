// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The key handling of `vault rekey` (ADR 0080).
//!
//! A rekey writes every note under a new key, so a crash must never leave a
//! note readable only by a key that exists in memory. The new keypair is
//! therefore **staged** in `.ntropy/` and flushed before the marker is
//! written, and a resume loads it instead of generating another. Only after
//! the last note is replaced does the staged pair take the place of
//! `identity.age` and `identity.pub`, the wrapped identity first: it is the
//! file that opens the notes, and from the moment it holds the new key
//! every note on disk is readable with it.

use std::path::{Path, PathBuf};

use super::marker::{self, Marker, Operation};
use super::plan::REKEY_SUFFIX;
use super::{MigrateError, Migration, MigrationHooks, MigrationReport, run};
use crate::cipher::{AgeCipher, NoteCipher, Passphrase};
use crate::crypto::{Identity, Recipient, age_io};
use crate::fsutil;
use crate::keys;
use crate::scan;
use crate::session::VaultSession;
use crate::vault::Layout;

/// Installing the staged keypair as the vault's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHandover {
    /// The recipient the marker names; the staged pair has to be this one.
    recipient: String,
    staged_pub: PathBuf,
    staged_wrapped: PathBuf,
    live_pub: PathBuf,
    live_wrapped: PathBuf,
}

impl KeyHandover {
    /// The handover of the pair staged in `layout`'s `.ntropy/` to `recipient`.
    pub fn new(layout: &Layout, recipient: &Recipient) -> Self {
        Self {
            recipient: recipient.as_str(),
            staged_pub: layout.identity_pub_staged(),
            staged_wrapped: layout.identity_file_staged(),
            live_pub: layout.identity_pub(),
            live_wrapped: layout.identity_file(),
        }
    }

    /// Fail unless the staged recipient is the one the marker names.
    ///
    /// Nothing else ties the staged files to this rekey: a second process, a
    /// sync tool restoring an older copy or a file moved by hand can replace
    /// them, and installing a key no note is encrypted to would leave every
    /// note unreadable.
    pub fn check_staged(&self) -> Result<(), MigrateError> {
        match keys::read_recipient_at(&self.staged_pub)? {
            Some(found) if found.as_str() == self.recipient => Ok(()),
            Some(found) => Err(MigrateError::StagedKeyMismatch {
                path: self.staged_pub.clone(),
                expected: self.recipient.clone(),
                found: found.as_str(),
            }),
            None => Err(MigrateError::StagedKeyMissing(vec![
                self.staged_pub.clone(),
            ])),
        }
    }

    /// Install the staged pair once every note has been replaced.
    ///
    /// Every note has to open with the new key first: the old key is about to
    /// stop being the vault's identity, and a note that opens with neither is
    /// then unreadable for good. The `handover` flag goes into the marker
    /// before the first identity file changes, so a resume knows the old key
    /// is no longer asked for.
    pub(super) fn install(
        &self,
        all_notes: &Path,
        target: &dyn NoteCipher,
        marker_path: &Path,
        marker: &Marker,
        hooks: &dyn MigrationHooks,
    ) -> Result<(), MigrateError> {
        let unreadable = unreadable_notes(all_notes, target)?;
        if !unreadable.is_empty() {
            return Err(MigrateError::UnreadableNotes(unreadable));
        }
        self.check_staged()?;

        // The removals and renames of the commit are directory entries of
        // `all-notes/`. The flag below is flushed in another directory, and
        // nothing orders the two on disk.
        fsutil::sync_dir(all_notes)?;

        let mut handed_over = marker.clone();
        handed_over.handover = true;
        marker::write_at(marker_path, &handed_over)?;

        self.finish(hooks)
    }

    /// Replace the vault's identity with the staged pair and remove the
    /// staged files.
    ///
    /// Each step looks at what is left, so it can run again from any point a
    /// crash stopped it at.
    pub fn finish(&self, hooks: &dyn MigrationHooks) -> Result<(), MigrateError> {
        if self.staged_pub.exists() {
            self.check_staged()?;
        }

        if self.staged_wrapped.exists() {
            let wrapped = fsutil::read(&self.staged_wrapped)?;
            fsutil::atomic_write_private(&self.live_wrapped, &wrapped)?;
            flush(&self.live_wrapped)?;
        }

        hooks.between_key_files()?;

        if self.staged_pub.exists() {
            let recipient = fsutil::read(&self.staged_pub)?;
            fsutil::atomic_write(&self.live_pub, &recipient)?;
            flush(&self.live_pub)?;
        }

        // The wrapped file goes first, so a crash leaves either both staged
        // files or the recipient alone, and the recipient alone is enough to
        // finish.
        for staged in [&self.staged_wrapped, &self.staged_pub] {
            if staged.exists() {
                fsutil::remove_file(staged)?;
            }
        }
        if let Some(dir) = self.live_pub.parent() {
            fsutil::sync_dir(dir)?;
        }
        Ok(())
    }
}

/// Flush a file and the directory entry that names it.
fn flush(path: &Path) -> Result<(), MigrateError> {
    fsutil::sync_file(path)?;
    if let Some(dir) = path.parent() {
        fsutil::sync_dir(dir)?;
    }
    Ok(())
}

/// The notes in `all_notes` that `cipher` cannot open.
fn unreadable_notes(
    all_notes: &Path,
    cipher: &dyn NoteCipher,
) -> Result<Vec<PathBuf>, MigrateError> {
    let scan = scan::scan_notes_dir(all_notes, cipher)?;
    Ok(scan
        .warnings
        .into_iter()
        .map(|warning| warning.path)
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == cipher.extension())
        })
        .collect())
}

/// The produced files whose source note is gone.
///
/// A crash between a commit's removal of the source and the rename of its
/// replacement leaves the note under its temporary name only.
fn orphans(all_notes: &Path) -> Result<Vec<PathBuf>, MigrateError> {
    let mut found: Vec<PathBuf> = fsutil::read_dir_entries(all_notes)?
        .into_iter()
        .filter(|(_, kind)| kind.is_file())
        .filter_map(|(path, _)| {
            let name = path.file_name()?.to_str()?;
            let final_name = name.strip_suffix(REKEY_SUFFIX)?;
            (!path.with_file_name(final_name).exists()).then_some(path)
        })
        .collect();
    found.sort();
    Ok(found)
}

/// Move every orphaned produced file to its final name.
///
/// A file `target` cannot read is left alone and reported, since renaming it
/// would present a note nothing can open as the note.
pub(super) fn adopt_orphans(all_notes: &Path, target: &dyn NoteCipher) -> Result<(), MigrateError> {
    let orphans = orphans(all_notes)?;
    let unreadable: Vec<PathBuf> = orphans
        .iter()
        .filter(|path| target.read(path).is_err())
        .cloned()
        .collect();
    if !unreadable.is_empty() {
        return Err(MigrateError::UnreadableNotes(unreadable));
    }

    for orphan in &orphans {
        let name = orphan.file_name().and_then(|name| name.to_str());
        let final_name = name
            .and_then(|name| name.strip_suffix(REKEY_SUFFIX))
            .expect("orphans are selected by this suffix");
        fsutil::rename(orphan, &orphan.with_file_name(final_name))?;
    }
    if !orphans.is_empty() {
        fsutil::sync_dir(all_notes)?;
    }
    Ok(())
}

/// What a finished rekey did.
#[derive(Debug)]
pub struct RekeyOutcome {
    /// The notes written, verified and replaced.
    pub report: MigrationReport,
    /// The recipient the vault uses now.
    pub recipient: Recipient,
}

/// Everything a rekey needs.
pub struct Rekey<'a> {
    pub session: &'a VaultSession,
    /// Reads the notes as they are now.
    pub source: &'a dyn NoteCipher,
    /// The vault's recipient before the rekey.
    pub source_recipient: &'a Recipient,
    /// Wraps the new identity when staging, and unwraps the staged one when
    /// resuming.
    pub passphrase: &'a Passphrase,
    /// Whether this run finishes an interrupted one.
    pub resume: bool,
    pub hooks: &'a dyn MigrationHooks,
    /// The marker's informational timestamp.
    pub started: String,
}

/// Rekey the vault, or finish an interrupted rekey.
pub fn rekey(request: &Rekey<'_>) -> Result<RekeyOutcome, MigrateError> {
    let layout = request.session.layout();
    let all_notes = layout.all_notes();
    let marker_path = layout.migration_file();

    request.source.readable()?;

    let existing = marker::read_at(&marker_path)?;
    match (&existing, request.resume) {
        (None, true) => return Err(MigrateError::NothingToResume),
        (Some(found), false) => {
            return Err(MigrateError::AlreadyInProgress {
                operation: found.operation,
                resume: found.operation.resume_command(),
            });
        }
        (Some(found), true) if found.operation != Operation::Rekey => {
            return Err(MigrateError::AlreadyInProgress {
                operation: found.operation,
                resume: found.operation.resume_command(),
            });
        }
        _ => {}
    }

    let (identity, marker) = match existing {
        None => stage_fresh(request, &all_notes)?,
        Some(found) if found.handover => {
            let recipient = recipient_of(&found)?;
            finish_handover(layout, request.hooks)?;
            return Ok(RekeyOutcome {
                report: MigrationReport {
                    resumed: true,
                    ..Default::default()
                },
                recipient,
            });
        }
        Some(found) if found.staged => (load_staged(request, layout, &found)?, found),
        Some(found) => restage(request, &all_notes, found)?,
    };

    let recipient = identity.to_public();
    let target = AgeCipher::new(recipient.clone(), Some(identity));
    let report = run(
        request.session,
        &Migration {
            operation: Operation::Rekey,
            source: request.source,
            target: &target,
            resume: request.resume,
            marker,
            hooks: request.hooks,
            handover: Some(KeyHandover::new(layout, &recipient)),
        },
    )?;
    Ok(RekeyOutcome { report, recipient })
}

/// The recipient a rekey's marker names as its target.
fn recipient_of(marker: &Marker) -> Result<Recipient, MigrateError> {
    marker
        .recipient
        .as_deref()
        .and_then(|text| text.parse().ok())
        .ok_or(MigrateError::MarkerWithoutRecipient)
}

/// Generate the new keypair and write it where a resume finds it.
fn stage_new_pair(request: &Rekey<'_>, layout: &Layout) -> Result<Identity, MigrateError> {
    let (identity, _) = age_io::generate_keypair();
    keys::stage_keypair_at(
        &layout.identity_pub_staged(),
        &layout.identity_file_staged(),
        &identity,
        request.passphrase,
    )?;
    Ok(identity)
}

/// The marker for a rekey to `identity`'s key, which is staged already.
fn staged_marker(request: &Rekey<'_>, identity: &Identity) -> Marker {
    let mut marker = Marker::new(
        Operation::Rekey,
        Some(identity.to_public().as_str()),
        request.started.clone(),
    );
    marker.source_recipient = Some(request.source_recipient.as_str());
    marker.staged = true;
    marker
}

/// Begin a rekey: refuse what it could not finish, then stage the new key.
fn stage_fresh(request: &Rekey<'_>, all_notes: &Path) -> Result<(Identity, Marker), MigrateError> {
    let layout = request.session.layout();

    super::preflight(all_notes, Operation::Rekey, request.source)?;

    // A note the old key cannot open would be skipped and left behind, then
    // lose its key when the old identity is replaced.
    let unreadable = unreadable_notes(all_notes, request.source)?;
    if !unreadable.is_empty() {
        return Err(MigrateError::UnreadableNotes(unreadable));
    }

    // A staged pair without a marker is normally the remains of a start that
    // stopped before the marker. A produced file without its note says it may
    // hold notes: only then is the pair worth keeping.
    let stranded = orphans(all_notes)?;
    let staged_present =
        layout.identity_pub_staged().exists() || layout.identity_file_staged().exists();
    if staged_present && !stranded.is_empty() {
        return Err(MigrateError::StrandedNotes(stranded));
    }

    let identity = stage_new_pair(request, layout)?;
    let marker = staged_marker(request, &identity);
    Ok((identity, marker))
}

/// The staged identity of an interrupted rekey.
fn load_staged(
    request: &Rekey<'_>,
    layout: &Layout,
    marker: &Marker,
) -> Result<Identity, MigrateError> {
    let recipient = recipient_of(marker)?;
    let handover = KeyHandover::new(layout, &recipient);

    let missing: Vec<PathBuf> = [layout.identity_pub_staged(), layout.identity_file_staged()]
        .into_iter()
        .filter(|path| !path.exists())
        .collect();
    if !missing.is_empty() {
        return Err(MigrateError::StagedKeyMissing(missing));
    }
    handover.check_staged()?;

    let wrapped = layout.identity_file_staged();
    let identity = keys::unwrap_identity_at(&wrapped, request.passphrase).map_err(|source| {
        MigrateError::StagedKeyLocked {
            path: wrapped.clone(),
            source,
        }
    })?;
    if identity.to_public() != recipient {
        return Err(MigrateError::StagedKeyMismatch {
            path: wrapped,
            expected: recipient.as_str(),
            found: identity.to_public().as_str(),
        });
    }
    Ok(identity)
}

/// Continue a rekey that never staged its key.
///
/// Its new key died with the process, so notes it already wrote cannot be
/// opened by anything. Without such notes the rekey starts over under a
/// staged key; with them it stops and names them.
fn restage(
    request: &Rekey<'_>,
    all_notes: &Path,
    found: Marker,
) -> Result<(Identity, Marker), MigrateError> {
    let layout = request.session.layout();

    let mut lost = unreadable_notes(all_notes, request.source)?;
    lost.extend(orphans(all_notes)?);
    if !lost.is_empty() {
        lost.sort();
        return Err(MigrateError::StrandedNotes(lost));
    }

    let identity = stage_new_pair(request, layout)?;
    let marker = Marker {
        started: found.started,
        ..staged_marker(request, &identity)
    };
    marker::write_at(&layout.migration_file(), &marker)?;
    Ok((identity, marker))
}

/// Finish the key handover of an interrupted rekey that reached it.
///
/// Needs no key: the notes are all encrypted to the staged one already.
/// Returns the recipient the vault used before the rekey.
pub fn finish_handover(
    layout: &Layout,
    hooks: &dyn MigrationHooks,
) -> Result<Option<Recipient>, MigrateError> {
    let marker_path = layout.migration_file();
    let Some(found) = marker::read_at(&marker_path)? else {
        return Err(MigrateError::NothingToResume);
    };
    let recipient = recipient_of(&found)?;
    KeyHandover::new(layout, &recipient).finish(hooks)?;
    marker::remove_at(&marker_path)?;
    Ok(found
        .source_recipient
        .as_deref()
        .and_then(|text| text.parse().ok()))
}

#[cfg(test)]
#[path = "rekey_tests.rs"]
mod tests;
