// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The write-note use case (ADR 0043).
//!
//! Replaces one existing note's content with text the caller supplies. This is
//! the counterpart to reading a note out through the vault's cipher: it is the
//! only way to put content into an encrypted note without an editor, and it
//! behaves identically in a plaintext vault, so a caller has one way to author
//! notes rather than one per storage form.
//!
//! Targeting is by name alone, never by scanning. A scan would have to read
//! every note, which on an encrypted vault needs the key; resolving
//! `<ulid>-<slug>.md` or `<ulid>.age` from the directory listing needs nothing,
//! so a locked vault can still be written to just as it can still be created in
//! (ADR 0041).

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::id::{Id, ULID_LEN};
use crate::note::{Note, filename};
use crate::session::VaultSession;

/// Why a write could not be directed at a note.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("no note `{0}` in the vault's `all-notes/`")]
    NotFound(String),
    #[error("`{0}` is not a note in the vault's `all-notes/`")]
    OutsideVault(String),
    #[error("`{0}` names {1} files; pass the full filename of the one you mean")]
    Ambiguous(String, usize),
}

/// Resolve `target` to the note file it names, without reading any note.
///
/// Three shapes are accepted, in this order:
///
/// - a bare 26-character ULID, matched against the identity every note filename
///   starts with;
/// - a filename inside the vault's `all-notes/`;
/// - a path, which must point into that same `all-notes/`.
///
/// The file must already exist. Allocating a new identity belongs to `new`,
/// which is what keeps a caller from inventing a ULID (ADR 0004).
pub fn locate(session: &VaultSession, target: &str) -> Result<PathBuf> {
    let all_notes = session.layout().all_notes();

    if let Some(id) = as_ulid(target) {
        return locate_by_id(&all_notes, target, &id);
    }

    // A separator makes it a path; anything else names a file directly in the
    // canonical store. Both end at the same place, which is what lets the path
    // `new --print` hands back be passed straight through.
    let path = if has_separator(target) {
        let path = PathBuf::from(target);
        if !in_all_notes(&path, &all_notes) {
            return Err(WriteError::OutsideVault(target.to_string()).into());
        }
        path
    } else {
        all_notes.join(target)
    };

    if !path.is_file() {
        return Err(WriteError::NotFound(target.to_string()).into());
    }
    Ok(path)
}

/// Find the one file in `all_notes` whose name carries `id`.
///
/// Both storage forms are recognized, so the same ULID resolves in a plaintext
/// and an encrypted vault alike. Duplicates are reported rather than picked
/// between: two files claiming one identity is a vault to repair, not a choice
/// to guess at.
fn locate_by_id(all_notes: &Path, target: &str, id: &Id) -> Result<PathBuf> {
    let Ok(entries) = std::fs::read_dir(all_notes) else {
        return Err(WriteError::NotFound(target.to_string()).into());
    };

    let mut found: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if filename::parse_any(name).is_ok_and(|parsed| &parsed.id == id) {
            found.push(entry.path());
        }
    }

    match found.len() {
        0 => Err(WriteError::NotFound(target.to_string()).into()),
        1 => Ok(found.remove(0)),
        n => Err(WriteError::Ambiguous(target.to_string(), n).into()),
    }
}

/// Parse `target` as a full ULID, or `None` if it is not one.
fn as_ulid(target: &str) -> Option<Id> {
    if target.len() == ULID_LEN {
        target.parse::<Id>().ok()
    } else {
        None
    }
}

/// Whether `target` names a path rather than a bare filename.
fn has_separator(target: &str) -> bool {
    Path::new(target).components().count() > 1
}

/// Whether `path` sits directly in `all_notes`.
///
/// Compared through the resolved parent directory so that `.`, `..` and a
/// symlinked vault root all land on the same answer as a plain path would.
fn in_all_notes(path: &Path, all_notes: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let parent = std::fs::canonicalize(parent);
    let all_notes = std::fs::canonicalize(all_notes);
    matches!((parent, all_notes), (Ok(p), Ok(a)) if p == a)
}

/// Replace the content of the note `target` names with `content`.
///
/// `content` is the note's whole file: frontmatter block and body. It is parsed
/// before anything is written, so text that is not a well-formed note is
/// refused rather than stored — the same validate-before-write order `new` uses
/// for a rendered template (ADR 0034). The identity is not read from `content`
/// and cannot be changed by it; it stays what the filename says (ADR 0004).
///
/// Returns the note as written. Its filename may still need realigning to a
/// changed title, which the caller does, since that is presentation the library
/// does not own.
pub fn write_note(session: &VaultSession, target: &str, content: &str) -> Result<Note> {
    let path = locate(session, target)?;

    let mut note = Note::parse(path.clone(), content, None)?;
    session.cipher().write(&path, content)?;

    note.modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    Ok(note)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{create_empty_note, create_note};
    use crate::vault::Vault;

    const NOTE: &str = "---\ntitle: Written\ntags: [written]\n---\n# Written\n\nBody.\n";

    fn temp_vault() -> (tempfile::TempDir, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect("mkdir .ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));
        (dir, session)
    }

    #[test]
    fn writing_fills_an_empty_note() {
        // The flow the command exists for: `new --empty` allocates, `write`
        // fills, and what comes back is a well-formed note.
        let (_guard, session) = temp_vault();
        let path = create_empty_note(&session, "Written").expect("create");

        let note = write_note(&session, path.to_str().expect("utf-8"), NOTE).expect("write");
        assert_eq!(note.title, "Written");
        assert_eq!(note.tags, vec!["written"]);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), NOTE);
    }

    #[test]
    fn writing_replaces_an_existing_notes_content() {
        let (_guard, session) = temp_vault();
        let created = create_note(&session, "Original", None).expect("create");

        write_note(&session, created.path.to_str().expect("utf-8"), NOTE).expect("write");
        let on_disk = std::fs::read_to_string(&created.path).expect("read");
        assert_eq!(on_disk, NOTE);
        assert!(!on_disk.contains("Original"), "old content survived");
    }

    #[test]
    fn a_bare_ulid_finds_the_note() {
        let (_guard, session) = temp_vault();
        let created = create_note(&session, "By Identity", None).expect("create");

        let note = write_note(&session, &created.id.to_string(), NOTE).expect("write");
        assert_eq!(note.path, created.path);
    }

    #[test]
    fn a_bare_filename_finds_the_note() {
        let (_guard, session) = temp_vault();
        let created = create_note(&session, "By Filename", None).expect("create");
        let name = created.path.file_name().expect("name").to_string_lossy();

        let note = write_note(&session, &name, NOTE).expect("write");
        assert_eq!(note.path, created.path);
    }

    #[test]
    fn a_relative_path_into_all_notes_finds_the_note() {
        // Whatever spelling reaches the same file is accepted; only the
        // directory it lands in matters.
        let (guard, session) = temp_vault();
        let created = create_note(&session, "By Path", None).expect("create");
        let name = created.path.file_name().expect("name").to_string_lossy();
        let relative = guard.path().join("all-notes/.").join(name.as_ref());

        let note = write_note(&session, relative.to_str().expect("utf-8"), NOTE).expect("write");
        assert_eq!(note.path, relative);
        assert_eq!(std::fs::read_to_string(&created.path).expect("read"), NOTE);
    }

    #[test]
    fn a_path_outside_all_notes_is_refused() {
        let (guard, session) = temp_vault();
        create_note(&session, "Bystander", None).expect("create");
        let outside = guard.path().join("elsewhere.md");
        std::fs::write(&outside, "---\ntitle: Elsewhere\n---\n").expect("write outside");

        let err = write_note(&session, outside.to_str().expect("utf-8"), NOTE)
            .expect_err("outside the vault");
        assert!(matches!(
            err,
            crate::error::Error::Write(WriteError::OutsideVault(_))
        ));
        // The file it pointed at is untouched.
        assert!(
            std::fs::read_to_string(&outside)
                .expect("read")
                .contains("Elsewhere")
        );
    }

    #[test]
    fn an_unknown_target_is_refused() {
        let (_guard, session) = temp_vault();
        create_note(&session, "Present", None).expect("create");

        for target in [
            "01KWVBW61WHJY7K27WNETSF641",
            "01KWVBW61WHJY7K27WNETSF641-no-such-note.md",
            "no-such-note.md",
        ] {
            let err = write_note(&session, target, NOTE).expect_err("missing note");
            assert!(
                matches!(err, crate::error::Error::Write(WriteError::NotFound(_))),
                "{target} produced {err:?}"
            );
        }
    }

    #[test]
    fn a_ulid_naming_two_files_is_refused_rather_than_guessed_at() {
        // Two files claiming one identity is a vault to repair. Picking one
        // would silently overwrite a note the caller never named.
        let (_guard, session) = temp_vault();
        let created = create_note(&session, "First Slug", None).expect("create");
        let twin = created
            .path
            .with_file_name(format!("{}-second-slug.md", created.id));
        std::fs::copy(&created.path, &twin).expect("copy");

        let err = write_note(&session, &created.id.to_string(), NOTE).expect_err("ambiguous");
        assert!(matches!(
            err,
            crate::error::Error::Write(WriteError::Ambiguous(_, 2))
        ));
    }

    #[test]
    fn writing_never_creates_a_note() {
        // Allocating an identity is `new`'s job; a write that missed its target
        // must not quietly invent one.
        let (guard, session) = temp_vault();
        std::fs::create_dir_all(guard.path().join("all-notes")).expect("all-notes");

        write_note(&session, "01KWVBW61WHJY7K27WNETSF641", NOTE).expect_err("missing note");
        assert_eq!(
            std::fs::read_dir(guard.path().join("all-notes"))
                .expect("read all-notes")
                .count(),
            0
        );
    }

    #[test]
    fn content_that_is_not_a_note_is_refused_before_anything_is_written() {
        let (_guard, session) = temp_vault();
        let created = create_note(&session, "Intact", None).expect("create");
        let before = std::fs::read_to_string(&created.path).expect("read");

        for bad in [
            "",
            "no frontmatter at all\n",
            "---\ntags: []\n---\nNo title.\n",
        ] {
            let err = write_note(&session, &created.id.to_string(), bad).expect_err("not a note");
            assert!(
                matches!(err, crate::error::Error::Note(_)),
                "{bad:?} produced {err:?}"
            );
        }

        assert_eq!(
            std::fs::read_to_string(&created.path).expect("read"),
            before,
            "a refused write must leave the note alone"
        );
    }

    #[test]
    fn the_identity_comes_from_the_filename_not_the_content() {
        // Frontmatter carries no identity (ADR 0005), so nothing a caller
        // writes can move a note onto another ULID.
        let (_guard, session) = temp_vault();
        let created = create_note(&session, "Stable", None).expect("create");

        let note = write_note(
            &session,
            &created.id.to_string(),
            "---\ntitle: Renamed\nid: 01KWVBW61WHJY7K27WNETSF641\n---\n# Renamed\n",
        )
        .expect("write");
        assert_eq!(note.id, created.id);
        assert_eq!(note.path, created.path);
    }

    /// Writing into a vault whose notes are encrypted.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::NOTE;
        use crate::ops::write::write_note;
        use crate::ops::{create_empty_note, create_note};
        use crate::test_support::encrypted::encrypted_vault;

        #[test]
        fn the_written_note_is_stored_as_ciphertext() {
            let fixture = encrypted_vault();
            let created = create_note(&fixture.session, "Secret Plans", None).expect("create");

            write_note(&fixture.session, &created.id.to_string(), NOTE).expect("write");

            let raw = std::fs::read(&created.path).expect("read bytes");
            assert!(
                !raw.windows(7).any(|w| w == b"Written"),
                "the title reached the file in the clear"
            );
            assert_eq!(
                fixture.session.cipher().read(&created.path).expect("read"),
                NOTE
            );
        }

        #[test]
        fn writing_works_on_a_locked_vault() {
            // The headline property this command had to preserve: targeting by
            // name reads no note, and encrypting needs only the public
            // recipient, so a machine without the key can still author.
            let fixture = encrypted_vault();
            let path = create_empty_note(&fixture.session, "Filled While Locked").expect("create");
            let locked = fixture.locked();
            assert!(!locked.is_unlocked());

            let name = path
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            write_note(&locked, &name, NOTE).expect("write while locked");

            assert_eq!(fixture.session.cipher().read(&path).expect("read"), NOTE);
        }

        #[test]
        fn a_written_note_reads_back_through_a_scan() {
            let fixture = encrypted_vault();
            let path = create_empty_note(&fixture.session, "Round Trip").expect("create");

            write_note(&fixture.session, path.to_str().expect("utf-8"), NOTE).expect("write");

            let matches = crate::ops::search(&fixture.session, None).expect("search");
            assert_eq!(matches.notes.len(), 1);
            assert_eq!(matches.notes[0].title, "Written");
        }
    }
}
