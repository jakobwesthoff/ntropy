// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What `reconcile` reports about assets (ADR 0099).
//!
//! Four things are wrong with an asset link or folder, and each fails
//! `--strict`: a link to a file missing from the note's own folder, a link
//! into another note's folder, a link to a file outside `all-notes/_assets/`,
//! and a folder in `_assets/` that belongs to no note. A folder
//! whose name is stale between an edit and the next `reconcile` is none of
//! them: the note owns it, and `reconcile` renames it before this runs.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::note::Note;
use crate::vault::layout::{ALL_NOTES_DIR, ASSETS_DIR, Layout};

use crate::render::markdown::resolve_root_relative;

use super::{Standing, html_ranges, local_links, owner, place, references};

/// One finding about a note's links or the contents of `_assets/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetIssue {
    /// A link into the note's own folder whose file is not there.
    Missing { note: PathBuf, path: String },
    /// A link into another note's folder, or into `_assets/` outside any
    /// folder of a note.
    Foreign { note: PathBuf, path: String },
    /// A link to a file that is in no asset folder.
    Unmanaged { note: PathBuf, path: String },
    /// A folder in `_assets/` that no valid note owns.
    Orphan { entry: String },
    /// A file directly in `_assets/`, outside every note's folder.
    Loose { entry: String },
    /// A symlink in or as `_assets/`, which ntropy leaves alone.
    Skipped { entry: String },
}

impl AssetIssue {
    /// The warning as one line. Names are not escaped here: the caller
    /// escapes the whole line before printing it (ADR 0071).
    pub fn message(&self) -> String {
        let file = |note: &Path| {
            note.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        match self {
            AssetIssue::Missing { note, path } => format!(
                "{}: `{path}` is linked but is not in the note's asset folder",
                file(note)
            ),
            AssetIssue::Foreign { note, path } => format!(
                "{}: `{path}` is in another note's asset folder, and a note has only its own assets",
                file(note)
            ),
            AssetIssue::Unmanaged { note, path } => format!(
                "{}: `{path}` is outside `{ALL_NOTES_DIR}/{ASSETS_DIR}/`, so ntropy does not manage it",
                file(note)
            ),
            AssetIssue::Orphan { entry } => {
                format!("`{ASSETS_DIR}/{entry}` belongs to no note: no valid note carries its ULID")
            }
            AssetIssue::Loose { entry } => format!(
                "`{ASSETS_DIR}/{entry}` is a file directly in `{ASSETS_DIR}/`, outside every note's folder"
            ),
            AssetIssue::Skipped { entry } => {
                format!("`{entry}` is a symlink, which ntropy leaves alone")
            }
        }
    }
}

/// Everything wrong with the assets of `notes` and with `_assets/` itself.
///
/// `notes` carry the bodies as they are after the rewrite of `reconcile`,
/// and the folder names as they are after its renames.
pub fn check(layout: &Layout, notes: &[Note]) -> Result<Vec<AssetIssue>> {
    let mut issues = check_folders(layout, notes)?;
    issues.extend(check_links(layout, notes));
    Ok(issues)
}

/// The entries of `_assets/` no note owns, and the symlinks among them.
fn check_folders(layout: &Layout, notes: &[Note]) -> Result<Vec<AssetIssue>> {
    let assets = layout.assets_dir();
    let is_symlink =
        |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    if is_symlink(&assets) {
        return Ok(vec![AssetIssue::Skipped {
            entry: format!("{ALL_NOTES_DIR}/{ASSETS_DIR}"),
        }]);
    }

    let owners: HashSet<Id> = notes.iter().map(|note| note.id).collect();
    let mut entries = fsutil::read_dir_entries(&assets)?;
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut issues = Vec::new();
    for (path, file_type) in entries {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if file_type.is_symlink() {
            issues.push(AssetIssue::Skipped {
                entry: format!("{ASSETS_DIR}/{name}"),
            });
        } else if !file_type.is_dir() {
            issues.push(AssetIssue::Loose { entry: name });
        } else if !owner(&name).is_some_and(|id| owners.contains(&id)) {
            issues.push(AssetIssue::Orphan { entry: name });
        }
    }
    Ok(issues)
}

/// The findings about each note's links, each file once per note.
fn check_links(layout: &Layout, notes: &[Note]) -> Vec<AssetIssue> {
    let note_dir = format!("/{ALL_NOTES_DIR}");
    let mut issues = Vec::new();
    for note in notes {
        let mut seen: BTreeSet<(Standing, String)> = BTreeSet::new();
        for link in local_links(&note.body) {
            let place = place(&note_dir, &link.dest);
            let standing = place.standing(note.id);
            let path = link.path().to_owned();
            if !seen.insert((standing, path.clone())) {
                continue;
            }
            let note = note.path.clone();
            let issue = match standing {
                Standing::Own => (!exists_in_vault(layout, &note_dir, &path))
                    .then_some(AssetIssue::Missing { note, path }),
                Standing::Foreign => Some(AssetIssue::Foreign { note, path }),
                Standing::Unmanaged => Some(AssetIssue::Unmanaged { note, path }),
                Standing::Escapes => None,
            };
            issues.extend(issue);
        }
        issues.extend(foreign_html_references(note, &mut seen));
    }
    issues
}

/// The references into another note's folder that raw HTML makes, which the
/// parser reads as HTML and not as links: an `<img src>` renders in a browser
/// and an export keeps it as written, so it is as much a use of the folder as
/// a link is.
fn foreign_html_references(
    note: &Note,
    seen: &mut BTreeSet<(Standing, String)>,
) -> Vec<AssetIssue> {
    let html = html_ranges(&note.body);
    references(&note.body)
        .into_iter()
        .filter(|reference| {
            html.iter()
                .any(|range| range.contains(&reference.name_range.start))
        })
        .filter(|reference| owner(&reference.name) != Some(note.id))
        .map(|reference| note.body[reference.token].to_owned())
        .filter(|path| seen.insert((Standing::Foreign, path.clone())))
        .map(|path| AssetIssue::Foreign {
            note: note.path.clone(),
            path,
        })
        .collect()
}

/// Whether the file `path`, a link target written in a note whose directory is
/// `note_dir`, exists in the vault, following symlinks, so a link to a
/// dangling symlink counts as missing.
fn exists_in_vault(layout: &Layout, note_dir: &str, path: &str) -> bool {
    resolve_root_relative(note_dir, path).is_some_and(|resolved| {
        std::fs::metadata(layout.root().join(resolved.trim_start_matches('/'))).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan;
    use crate::session::VaultSession;
    use crate::test_support::{vault_with_view, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";
    const GONE: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";

    fn note(session: &VaultSession, ulid: &str, body: &str) -> PathBuf {
        write_note(
            session,
            &format!("{ulid}-note.md"),
            &format!("---\ntitle: Note\n---\n{body}\n"),
        )
    }

    fn plant(session: &VaultSession, folder: &str, file: &str) {
        let folder = session.layout().asset_folder(folder);
        std::fs::create_dir_all(&folder).expect("asset folder");
        std::fs::write(folder.join(file), b"asset").expect("asset");
    }

    fn issues(session: &VaultSession) -> Vec<AssetIssue> {
        let scan =
            scan::scan_notes_dir(&session.layout().all_notes(), session.cipher()).expect("scan");
        check(session.layout(), &scan.notes).expect("check")
    }

    fn messages(session: &VaultSession) -> Vec<String> {
        issues(session).iter().map(AssetIssue::message).collect()
    }

    #[test]
    fn a_link_to_a_file_in_the_own_folder_is_fine() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, &format!("![c](_assets/{ULID}-note/c.png)"));
        plant(&session, &format!("{ULID}-note"), "c.png");
        assert_eq!(issues(&session), []);
    }

    #[test]
    fn a_link_to_the_own_folder_itself_is_fine() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, &format!("[all](_assets/{ULID}-note/)"));
        plant(&session, &format!("{ULID}-note"), "c.png");
        assert_eq!(issues(&session), []);
    }

    #[test]
    fn a_link_to_a_file_missing_from_the_own_folder_is_reported() {
        let (_guard, session) = vault_with_view();
        let path = note(&session, ULID, &format!("![c](_assets/{ULID}-note/c.png)"));
        plant(&session, &format!("{ULID}-note"), "other.png");
        assert_eq!(
            issues(&session),
            [AssetIssue::Missing {
                note: path,
                path: format!("_assets/{ULID}-note/c.png"),
            }]
        );
    }

    #[test]
    fn a_link_into_a_folder_that_does_not_exist_is_missing() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, &format!("![c](_assets/{ULID}-note/c.png)"));
        assert_eq!(messages(&session).len(), 1);
    }

    #[test]
    fn a_link_into_another_notes_folder_is_foreign() {
        let (_guard, session) = vault_with_view();
        let path = note(&session, ULID, &format!("![c](_assets/{OTHER}-note/c.png)"));
        note(&session, OTHER, "body");
        plant(&session, &format!("{OTHER}-note"), "c.png");
        assert_eq!(
            issues(&session),
            [AssetIssue::Foreign {
                note: path,
                path: format!("_assets/{OTHER}-note/c.png"),
            }]
        );
    }

    #[test]
    fn a_link_into_a_folder_of_no_note_is_foreign() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "[l](_assets/shared/logo.png)");
        plant(&session, "shared", "logo.png");
        assert!(
            issues(&session)
                .iter()
                .any(|issue| matches!(issue, AssetIssue::Foreign { .. }))
        );
    }

    #[test]
    fn a_link_to_a_file_outside_every_assets_folder_is_unmanaged() {
        let (_guard, session) = vault_with_view();
        let path = note(
            &session,
            ULID,
            "[t](transcript.txt) ![l](../assets/logo.svg)",
        );
        assert_eq!(
            issues(&session),
            [
                AssetIssue::Unmanaged {
                    note: path.clone(),
                    path: "transcript.txt".to_string(),
                },
                AssetIssue::Unmanaged {
                    note: path,
                    path: "../assets/logo.svg".to_string(),
                },
            ]
        );
    }

    #[test]
    fn a_file_linked_twice_is_reported_once() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "[a](t.txt) [b](t.txt#part)");
        assert_eq!(messages(&session).len(), 1);
    }

    #[test]
    fn urls_note_links_code_and_escaping_paths_are_not_reported() {
        let (_guard, session) = vault_with_view();
        note(
            &session,
            ULID,
            &format!(
                "[u](https://example.org/a.png) [n]({OTHER}-note.md) `[c](t.txt)` [e](../../outside.png) [h](#heading)"
            ),
        );
        note(&session, OTHER, "body");
        assert_eq!(issues(&session), []);
    }

    #[test]
    fn a_folder_no_note_owns_is_an_orphan() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        plant(&session, &format!("{GONE}-gone"), "c.png");
        assert_eq!(
            issues(&session),
            [AssetIssue::Orphan {
                entry: format!("{GONE}-gone"),
            }]
        );
    }

    #[test]
    fn a_folder_without_a_ulid_and_a_loose_file_are_orphans() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        plant(&session, "shared", "logo.png");
        std::fs::write(session.layout().assets_dir().join("loose.png"), b"x").expect("file");
        let orphans: Vec<String> = issues(&session)
            .into_iter()
            .filter_map(|issue| match issue {
                AssetIssue::Orphan { entry } => Some(entry),
                _ => None,
            })
            .collect();
        assert_eq!(orphans, ["shared"]);
    }

    #[test]
    fn a_file_directly_in_assets_is_loose_not_an_orphan_folder() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        std::fs::create_dir_all(session.layout().assets_dir()).expect("assets");
        std::fs::write(session.layout().assets_dir().join("loose.png"), b"x").expect("file");
        assert_eq!(
            issues(&session),
            [AssetIssue::Loose {
                entry: "loose.png".to_string()
            }]
        );
        assert_eq!(
            messages(&session),
            ["`_assets/loose.png` is a file directly in `_assets/`, outside every note's folder"]
        );
    }

    #[test]
    fn raw_html_into_another_notes_folder_is_foreign() {
        let (_guard, session) = vault_with_view();
        let path = note(
            &session,
            ULID,
            &format!(
                "<img src=\"_assets/{OTHER}-note/c.png\"> and prose _assets/{OTHER}-note/p.png"
            ),
        );
        note(&session, OTHER, "body");
        assert_eq!(
            issues(&session),
            [AssetIssue::Foreign {
                note: path,
                path: format!("_assets/{OTHER}-note/c.png"),
            }]
        );
    }

    #[test]
    fn raw_html_into_the_own_folder_is_not_reported() {
        let (_guard, session) = vault_with_view();
        note(
            &session,
            ULID,
            &format!("<img src=\"_assets/{ULID}-note/c.png\">"),
        );
        assert_eq!(issues(&session), []);
    }

    #[test]
    fn the_orphan_message_says_no_valid_note_carries_the_ulid() {
        let issue = AssetIssue::Orphan {
            entry: "shared".to_string(),
        };
        assert_eq!(
            issue.message(),
            "`_assets/shared` belongs to no note: no valid note carries its ULID"
        );
    }

    #[test]
    fn a_hidden_entry_is_ignored() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        std::fs::create_dir_all(session.layout().assets_dir()).expect("assets");
        std::fs::write(session.layout().assets_dir().join(".DS_Store"), b"x").expect("file");
        assert_eq!(issues(&session), []);
    }

    #[test]
    fn a_stale_named_folder_of_a_note_is_not_an_orphan() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        plant(&session, &format!("{ULID}-an-old-title"), "c.png");
        assert_eq!(issues(&session), []);
    }

    #[test]
    fn a_symlinked_entry_is_skipped_not_followed() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        std::fs::create_dir_all(session.layout().assets_dir()).expect("assets");
        let link = session.layout().assets_dir().join(format!("{ULID}-note"));
        std::os::unix::fs::symlink("/", link).expect("symlink");
        assert_eq!(
            issues(&session),
            [AssetIssue::Skipped {
                entry: format!("_assets/{ULID}-note"),
            }]
        );
    }

    #[test]
    fn a_symlinked_assets_directory_is_skipped_as_a_whole() {
        let (_guard, session) = vault_with_view();
        note(&session, ULID, "body");
        let elsewhere = session.root().join("elsewhere");
        std::fs::create_dir_all(elsewhere.join(format!("{GONE}-gone"))).expect("folder");
        std::os::unix::fs::symlink(&elsewhere, session.layout().assets_dir()).expect("symlink");
        assert_eq!(
            issues(&session),
            [AssetIssue::Skipped {
                entry: "all-notes/_assets".to_string(),
            }]
        );
    }

    #[test]
    fn the_messages_name_the_note_and_the_path() {
        let note = PathBuf::from("/vault/all-notes/n.md");
        let missing = AssetIssue::Missing {
            note: note.clone(),
            path: "_assets/f/c.png".to_string(),
        };
        let foreign = AssetIssue::Foreign {
            note: note.clone(),
            path: "_assets/g/c.png".to_string(),
        };
        let unmanaged = AssetIssue::Unmanaged {
            note,
            path: "t.txt".to_string(),
        };
        assert_eq!(
            missing.message(),
            "n.md: `_assets/f/c.png` is linked but is not in the note's asset folder"
        );
        assert_eq!(
            foreign.message(),
            "n.md: `_assets/g/c.png` is in another note's asset folder, and a note has only its own assets"
        );
        assert_eq!(
            unmanaged.message(),
            "n.md: `t.txt` is outside `all-notes/_assets/`, so ntropy does not manage it"
        );
        assert_eq!(
            AssetIssue::Skipped {
                entry: "_assets/x".to_string()
            }
            .message(),
            "`_assets/x` is a symlink, which ntropy leaves alone"
        );
    }
}
