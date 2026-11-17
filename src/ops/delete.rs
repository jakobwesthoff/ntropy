// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The delete use case (ADR 0018, ADR 0102).
//!
//! Removes a note's canonical file, then the asset folder the note owns when
//! no other note links into it, and then refreshes the views, so the deleted
//! note's links disappear from every view tree. Selector resolution and the
//! confirmation prompt live in the binary; this is the headless effect.

use std::path::{Path, PathBuf};

use crate::asset::{self, Place, local_links, owner};
use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::reconcile;
use crate::scan;
use crate::session::VaultSession;
use crate::vault::layout::ALL_NOTES_DIR;

/// One asset folder a note owns and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFolder {
    pub path: PathBuf,
    /// The files in it, hidden ones left out, at any depth.
    pub files: usize,
    /// Their size together.
    pub bytes: u64,
}

/// What deleting a note would do to its assets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeletePlan {
    /// The asset folders the note owns.
    pub folders: Vec<AssetFolder>,
    /// The other notes that link a file in one of those folders.
    pub referrers: Vec<PathBuf>,
}

impl DeletePlan {
    /// The files in all the folders.
    pub fn files(&self) -> usize {
        self.folders.iter().map(|folder| folder.files).sum()
    }

    /// Their size together.
    pub fn bytes(&self) -> u64 {
        self.folders.iter().map(|folder| folder.bytes).sum()
    }
}

/// What deleting the note `id` would do to the asset folders it owns.
///
/// An encrypted vault has no managed assets and plans nothing.
pub fn plan_delete(session: &VaultSession, id: Id) -> Result<DeletePlan> {
    if session.is_encrypted() {
        return Ok(DeletePlan::default());
    }
    let layout = session.layout();
    let mut folders = Vec::new();
    for name in asset::read_folders(layout)? {
        if owner(&name) == Some(id) {
            folders.push(measure(&layout.asset_folder(&name))?);
        }
    }
    if folders.is_empty() {
        return Ok(DeletePlan::default());
    }

    // A referrer is found by the ULID of the folder it links into and not by
    // its name, so a link with a stale slug or another letter case, which
    // `reconcile` would still repair to this folder, counts.
    let scan = scan::scan_notes_dir(&layout.all_notes(), session.cipher())?;
    let note_dir = format!("/{ALL_NOTES_DIR}");
    let mut referrers: Vec<PathBuf> = scan
        .notes
        .iter()
        .filter(|note| note.id != id)
        .filter(|note| {
            local_links(&note.body).iter().any(|link| {
                matches!(
                    asset::place(&note_dir, &link.dest),
                    Place::Managed { folder, .. } if owner(&folder) == Some(id)
                )
            })
        })
        .map(|note| note.path.clone())
        .collect();
    referrers.sort();
    Ok(DeletePlan { folders, referrers })
}

/// The files and bytes below `folder`, hidden entries left out. A symlink
/// counts as a file of no size and is never followed.
fn measure(folder: &Path) -> Result<AssetFolder> {
    let mut found = AssetFolder {
        path: folder.to_path_buf(),
        files: 0,
        bytes: 0,
    };
    let mut pending = vec![folder.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for (path, file_type) in fsutil::read_dir_entries(&dir)? {
            let hidden = path
                .file_name()
                .is_some_and(|name| name.as_encoded_bytes().starts_with(b"."));
            if hidden {
                continue;
            }
            if file_type.is_dir() {
                pending.push(path);
            } else {
                found.files += 1;
                if file_type.is_file() {
                    found.bytes += std::fs::symlink_metadata(&path)
                        .map(|metadata| metadata.len())
                        .unwrap_or(0);
                }
            }
        }
    }
    Ok(found)
}

/// Delete the note file at `path`, remove the asset `folders`, and sync the
/// views.
///
/// The note goes first: a crash between the two leaves a folder that
/// `reconcile` reports, never a note whose links point nowhere.
///
/// Returns what the view refresh left to report: the scan warnings (so a
/// caller can honor `--strict`) and any views it skipped.
pub fn delete_note(
    session: &VaultSession,
    path: &Path,
    folders: &[PathBuf],
) -> Result<reconcile::ViewRefresh> {
    fsutil::remove_file(path)?;
    for folder in folders {
        fsutil::remove_dir_all(folder)?;
    }
    reconcile::refresh_views(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{vault_with_view, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn id() -> Id {
        ULID.parse().expect("a valid ULID")
    }

    fn plant(session: &VaultSession, folder: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let folder = session.layout().asset_folder(folder);
        for (name, content) in files {
            let file = folder.join(name);
            std::fs::create_dir_all(file.parent().expect("a parent")).expect("directories");
            std::fs::write(file, content).expect("asset");
        }
        folder
    }

    fn note_linking(session: &VaultSession, ulid: &str, target: &str) -> PathBuf {
        write_note(
            session,
            &format!("{ulid}-linker.md"),
            &format!("---\ntitle: Linker\n---\n![x]({target})\n"),
        )
    }

    #[test]
    fn removes_file_and_prunes_links() {
        let (_g, session) = vault_with_view();
        let path = write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [work]\n---\n",
        );

        // Build the view first so there is a link to prune.
        reconcile::refresh_views(&session).expect("refresh");
        assert!(session.root().join("by-tag/work").is_dir());

        delete_note(&session, &path, &[]).expect("delete");
        assert!(!path.exists());
        // The note's group is gone after the sync.
        assert!(!session.root().join("by-tag/work").exists());
    }

    #[test]
    fn the_plan_counts_the_files_and_bytes_of_the_notes_folder() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        let folder = plant(
            &session,
            &format!("{ULID}-note"),
            &[
                ("a.png", b"12345"),
                ("sub/b.pdf", b"123"),
                (".DS_Store", b"x"),
            ],
        );

        let plan = plan_delete(&session, id()).expect("plan");

        assert_eq!(
            plan.folders,
            [AssetFolder {
                path: folder,
                files: 2,
                bytes: 8
            }]
        );
        assert_eq!((plan.files(), plan.bytes()), (2, 8));
        assert!(plan.referrers.is_empty());
    }

    #[test]
    fn the_plan_finds_the_folder_under_a_stale_name() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        plant(
            &session,
            &format!("{ULID}-an-older-title"),
            &[("a.png", b"1")],
        );

        let plan = plan_delete(&session, id()).expect("plan");

        assert_eq!(plan.folders.len(), 1);
    }

    #[test]
    fn a_note_without_a_folder_has_an_empty_plan() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        plant(&session, &format!("{OTHER}-other"), &[("a.png", b"1")]);

        assert_eq!(
            plan_delete(&session, id()).expect("plan"),
            DeletePlan::default()
        );
    }

    #[test]
    fn another_note_linking_a_file_in_the_folder_is_a_referrer() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);
        let linker = note_linking(&session, OTHER, &format!("_assets/{ULID}-note/a.png"));

        let plan = plan_delete(&session, id()).expect("plan");

        assert_eq!(plan.referrers, [linker]);
    }

    #[test]
    fn a_referrer_with_a_stale_slug_or_another_spelling_still_counts() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);
        let stale = note_linking(&session, OTHER, &format!("_assets/{ULID}-old-slug/a.png"));
        let third = "01CRZ3NDEKTSV4RRFFQ69G5FAV";
        let spelled = note_linking(&session, third, &format!("_ASSETS/{ULID}-note/a.png"));

        let plan = plan_delete(&session, id()).expect("plan");

        assert_eq!(plan.referrers, [stale, spelled]);
    }

    #[test]
    fn the_note_itself_and_unrelated_links_are_no_referrers() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            &format!("---\ntitle: Note\n---\n![x](_assets/{ULID}-note/a.png)\n"),
        );
        plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);
        note_linking(
            &session,
            OTHER,
            "_assets/01CRZ3NDEKTSV4RRFFQ69G5FAV-x/a.png",
        );

        assert!(
            plan_delete(&session, id())
                .expect("plan")
                .referrers
                .is_empty()
        );
    }

    #[test]
    fn a_symlink_in_the_folder_counts_as_a_file_and_adds_no_size() {
        let (_g, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        let folder = plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);
        let elsewhere = tempfile::tempdir().expect("temp dir");
        std::fs::write(elsewhere.path().join("big.bin"), vec![0u8; 4096]).expect("file");
        std::os::unix::fs::symlink(elsewhere.path().join("big.bin"), folder.join("big.bin"))
            .expect("symlink");

        let plan = plan_delete(&session, id()).expect("plan");

        assert_eq!((plan.files(), plan.bytes()), (2, 1));
    }

    #[test]
    fn the_folders_given_are_removed_with_the_note() {
        let (_g, session) = vault_with_view();
        let path = write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        let folder = plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);

        delete_note(&session, &path, std::slice::from_ref(&folder)).expect("delete");

        assert!(!path.exists() && !folder.exists());
    }

    #[test]
    fn a_folder_not_given_stays() {
        let (_g, session) = vault_with_view();
        let path = write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        let folder = plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);

        delete_note(&session, &path, &[]).expect("delete");

        assert!(!path.exists() && folder.join("a.png").exists());
    }

    #[test]
    fn removing_a_folder_never_follows_a_symlink_in_it() {
        let (_g, session) = vault_with_view();
        let path = write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        let folder = plant(&session, &format!("{ULID}-note"), &[("a.png", b"1")]);
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let kept = elsewhere.path().join("keep.txt");
        std::fs::write(&kept, "keep").expect("file");
        std::os::unix::fs::symlink(elsewhere.path(), folder.join("outside")).expect("symlink");

        delete_note(&session, &path, std::slice::from_ref(&folder)).expect("delete");

        assert!(!folder.exists());
        assert_eq!(std::fs::read_to_string(kept).expect("kept"), "keep");
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn an_encrypted_vault_plans_nothing() {
        let fixture = crate::test_support::encrypted::encrypted_vault();
        let session = &fixture.session;
        plant(session, &format!("{ULID}-note"), &[("a.png", b"1")]);

        assert_eq!(
            plan_delete(session, id()).expect("plan"),
            DeletePlan::default()
        );
    }
}
