// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Finding the note file a name refers to (ADRs 0043, 0091).
//!
//! `write` and the selector of `search`, `delete`, `render` and `show` accept
//! the same names for a note: a full ULID, a filename in the vault's
//! `all-notes/`, or a path to such a file. A symlink to a note, a view entry
//! being the usual case, names the note it points to.
//!
//! Resolving a name reads the directory listing and file metadata only, never
//! a note. A locked vault can therefore still say that a file is no note, and
//! `write` can still reach a note in it (ADR 0041).

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::id::{Id, ULID_LEN};
use crate::note::filename;
use crate::session::VaultSession;

/// Why a name could not be directed at a note.
#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("no note `{0}` in the vault's `all-notes/`")]
    NotFound(String),
    #[error("`{0}` is not a note in the vault's `all-notes/`")]
    OutsideVault(String),
    #[error("`{0}` names {1} files; pass the full filename of the one you mean")]
    Ambiguous(String, usize),
}

/// What a filename or path names.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Named {
    /// A note file directly in `all-notes/`, as the path to open it by.
    Note(PathBuf),
    /// An existing file that is no note of this vault: elsewhere, or not
    /// stored the way the vault stores its notes.
    NotANote,
    /// Nothing exists at the name.
    Missing,
}

/// Resolve `target` to the note file it names, without reading any note.
///
/// Three shapes are accepted, in this order:
///
/// - a bare 26-character ULID, matched against the identity every note filename
///   starts with;
/// - a filename inside the vault's `all-notes/`;
/// - a path to a note file there, or a symlink to one.
///
/// The file must already exist. Allocating a new identity belongs to `new`,
/// which is what keeps a caller from inventing a ULID (ADR 0004).
pub fn locate(session: &VaultSession, target: &str) -> Result<PathBuf> {
    if let Some(id) = as_ulid(target) {
        return locate_by_id(&session.layout().all_notes(), target, &id);
    }

    match name_file(session, target) {
        Named::Note(path) => Ok(path),
        Named::NotANote => Err(TargetError::OutsideVault(target.to_string()).into()),
        Named::Missing => Err(TargetError::NotFound(target.to_string()).into()),
    }
}

/// What the filename or path `target` names.
///
/// A name without a directory part is looked up in `all-notes/`, anything else
/// is taken as given, relative to the working directory or absolute. Only a
/// file stored the way the vault stores its notes counts: `.md` in a plaintext
/// vault, `.age` in an encrypted one.
pub(crate) fn name_file(session: &VaultSession, target: &str) -> Named {
    let all_notes = session.layout().all_notes();

    // A separator makes it a path; anything else names a file directly in the
    // canonical store. Both end at the same place, which is what lets the path
    // `new --print` hands back be passed straight through.
    let path = if has_separator(target) {
        PathBuf::from(target)
    } else {
        all_notes.join(target)
    };
    if !path.is_file() {
        return Named::Missing;
    }

    // A symlink is followed to the file it points to. Writing through the
    // link's own path would replace the link, so the note is named by its place
    // in `all-notes/`.
    let is_link = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink());
    // A link that exists but cannot be resolved was judged as it stands.
    let real = if is_link {
        std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone())
    } else {
        path.clone()
    };

    let stored_as_notes = real
        .extension()
        .is_some_and(|extension| extension == session.cipher().extension());
    if !stored_as_notes || !in_all_notes(&real, &all_notes) {
        return Named::NotANote;
    }

    match (is_link, real.file_name()) {
        (true, Some(name)) => Named::Note(all_notes.join(name)),
        _ => Named::Note(path),
    }
}

/// Whether the last component of `target` is a note's file name in either
/// storage form, `<ulid>-<slug>.md` or `<ulid>.age`.
pub(crate) fn is_note_file_name(target: &str) -> bool {
    Path::new(target)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| filename::parse_any(name).is_ok())
}

/// Whether `target` names a path rather than a bare filename.
pub(crate) fn has_separator(target: &str) -> bool {
    Path::new(target).components().count() > 1
}

/// Parse `target` as a full ULID, or `None` if it is not one.
pub(crate) fn as_ulid(target: &str) -> Option<Id> {
    if target.len() == ULID_LEN {
        target.parse::<Id>().ok()
    } else {
        None
    }
}

/// Find the one file in `all_notes` whose name carries `id`.
///
/// Both storage forms are recognized, so the same ULID resolves in a plaintext
/// and an encrypted vault alike. Duplicates are reported rather than picked
/// between: two files claiming one identity is a vault to repair, not a choice
/// to guess at.
fn locate_by_id(all_notes: &Path, target: &str, id: &Id) -> Result<PathBuf> {
    let Ok(entries) = std::fs::read_dir(all_notes) else {
        return Err(TargetError::NotFound(target.to_string()).into());
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
        0 => Err(TargetError::NotFound(target.to_string()).into()),
        1 => Ok(found.remove(0)),
        n => Err(TargetError::Ambiguous(target.to_string(), n).into()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{vault_with_view, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const NOTE: &str = "---\ntitle: Plan\ntags: [work]\n---\n";

    fn named(session: &VaultSession, target: &str) -> Named {
        name_file(session, target)
    }

    #[test]
    fn a_bare_filename_is_looked_up_in_all_notes() {
        let (_guard, session) = vault_with_view();
        let path = write_note(&session, &format!("{ULID}-plan.md"), NOTE);
        assert_eq!(
            named(&session, &format!("{ULID}-plan.md")),
            Named::Note(path)
        );
    }

    #[test]
    fn a_bare_name_that_is_not_in_all_notes_is_missing() {
        let (guard, session) = vault_with_view();
        std::fs::write(guard.path().join("README.md"), "# About\n").expect("write");
        assert_eq!(named(&session, "README.md"), Named::Missing);
    }

    #[test]
    fn a_path_to_a_note_names_it_as_given() {
        let (guard, session) = vault_with_view();
        let path = write_note(&session, &format!("{ULID}-plan.md"), NOTE);
        let spelled = guard
            .path()
            .join("all-notes/.")
            .join(format!("{ULID}-plan.md"));
        assert_eq!(
            named(&session, spelled.to_str().expect("utf-8")),
            Named::Note(spelled.clone())
        );
        assert!(path.exists());
    }

    #[test]
    fn files_outside_all_notes_are_not_notes() {
        let (guard, session) = vault_with_view();
        let readme = guard.path().join("README.md");
        std::fs::write(&readme, "# About\n").expect("write");
        std::fs::create_dir_all(guard.path().join(".ntropy/templates")).expect("templates");
        let template = guard.path().join(".ntropy/templates/default.md");
        std::fs::write(&template, "---\ntitle: {{title}}\n---\n").expect("write");

        for path in [readme, template] {
            assert_eq!(
                named(&session, path.to_str().expect("utf-8")),
                Named::NotANote
            );
        }
    }

    #[test]
    fn nothing_at_the_name_is_missing() {
        let (guard, session) = vault_with_view();
        let gone = guard.path().join(format!("all-notes/{ULID}-gone.md"));
        assert_eq!(
            named(&session, gone.to_str().expect("utf-8")),
            Named::Missing
        );
        assert_eq!(named(&session, "gone.md"), Named::Missing);
    }

    #[test]
    fn a_directory_is_missing() {
        let (guard, session) = vault_with_view();
        let directory = guard.path().join("all-notes/looks-like-a-note.md");
        std::fs::create_dir_all(&directory).expect("mkdir");
        assert_eq!(
            named(&session, directory.to_str().expect("utf-8")),
            Named::Missing
        );
    }

    #[test]
    fn a_file_stored_the_way_the_vault_does_not_store_notes_is_not_a_note() {
        let (_guard, session) = vault_with_view();
        write_note(&session, &format!("{ULID}.age"), "not a note");
        assert_eq!(named(&session, &format!("{ULID}.age")), Named::NotANote);
    }

    #[cfg(unix)]
    mod symlinks {
        use super::*;
        use std::os::unix::fs::symlink;

        #[test]
        fn a_view_entry_names_the_note_it_points_to() {
            let (guard, session) = vault_with_view();
            let path = write_note(&session, &format!("{ULID}-plan.md"), NOTE);
            crate::reconcile::refresh_views(&session).expect("refresh");
            let entry = std::fs::read_dir(guard.path().join("by-tag/work"))
                .expect("the tag's directory")
                .next()
                .expect("a link")
                .expect("entry")
                .path();
            assert!(
                std::fs::symlink_metadata(&entry)
                    .expect("stat")
                    .file_type()
                    .is_symlink()
            );

            assert_eq!(
                named(&session, entry.to_str().expect("utf-8")),
                Named::Note(path)
            );
        }

        #[test]
        fn a_symlink_from_outside_the_vault_names_the_note() {
            let (_guard, session) = vault_with_view();
            let path = write_note(&session, &format!("{ULID}-plan.md"), NOTE);
            let elsewhere = tempfile::tempdir().expect("temp dir");
            let link = elsewhere.path().join("shortcut.md");
            symlink(&path, &link).expect("symlink");
            assert_eq!(
                named(&session, link.to_str().expect("utf-8")),
                Named::Note(path)
            );
        }

        #[test]
        fn a_symlink_in_all_notes_names_the_note_it_points_to() {
            let (_guard, session) = vault_with_view();
            let path = write_note(&session, &format!("{ULID}-plan.md"), NOTE);
            let alias = session.layout().all_notes().join("alias.md");
            symlink(&path, &alias).expect("symlink");
            assert_eq!(
                named(&session, alias.to_str().expect("utf-8")),
                Named::Note(path)
            );
        }

        #[test]
        fn a_symlink_that_leaves_all_notes_is_not_a_note() {
            let (guard, session) = vault_with_view();
            let readme = guard.path().join("README.md");
            std::fs::write(&readme, "# About\n").expect("write");
            let link = session.layout().all_notes().join("leaving.md");
            symlink(&readme, &link).expect("symlink");
            assert_eq!(
                named(&session, link.to_str().expect("utf-8")),
                Named::NotANote
            );
        }

        #[test]
        fn a_dangling_symlink_is_missing() {
            let (guard, session) = vault_with_view();
            let link = guard.path().join("dangling.md");
            symlink(guard.path().join("all-notes/gone.md"), &link).expect("symlink");
            assert_eq!(
                named(&session, link.to_str().expect("utf-8")),
                Named::Missing
            );
        }
    }

    #[test]
    fn a_ulid_in_a_vault_without_notes_is_not_found() {
        let dir = tempfile::tempdir().expect("temp dir");
        let session = VaultSession::plaintext(crate::vault::Vault::new(dir.path()));
        let err = locate(&session, ULID).expect_err("nothing to find");
        assert!(matches!(
            err,
            crate::error::Error::Target(TargetError::NotFound(_))
        ));
    }

    #[test]
    fn a_path_without_a_parent_is_in_no_directory() {
        assert!(!in_all_notes(Path::new("/"), Path::new("/")));
    }

    #[test]
    fn note_file_names_have_a_ulid_in_either_storage_form() {
        assert!(is_note_file_name(&format!("{ULID}-plan.md")));
        assert!(is_note_file_name(&format!("{ULID}.age")));
        assert!(is_note_file_name(&format!("/v/all-notes/{ULID}-plan.md")));
        assert!(!is_note_file_name("notes-2026.md"));
        assert!(!is_note_file_name("README.md"));
        assert!(!is_note_file_name(&format!("{ULID}.md")));
    }

    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use crate::test_support::encrypted::encrypted_vault;

        #[test]
        fn an_age_file_is_a_note_of_an_encrypted_vault() {
            let fixture = encrypted_vault();
            let path = write_note(&fixture.session, &format!("{ULID}.age"), "ciphertext");
            assert_eq!(
                named(&fixture.session, &format!("{ULID}.age")),
                Named::Note(path)
            );
        }

        #[test]
        fn a_plaintext_note_file_is_not_a_note_of_an_encrypted_vault_yet() {
            let fixture = encrypted_vault();
            write_note(&fixture.session, &format!("{ULID}-plan.md"), NOTE);
            assert_eq!(
                named(&fixture.session, &format!("{ULID}-plan.md")),
                Named::NotANote
            );
        }

        #[test]
        fn a_locked_vault_resolves_a_name_without_its_key() {
            let fixture = encrypted_vault();
            let path = write_note(&fixture.session, &format!("{ULID}.age"), "ciphertext");
            let locked = fixture.locked();
            assert_eq!(named(&locked, &format!("{ULID}.age")), Named::Note(path));
        }
    }
}
