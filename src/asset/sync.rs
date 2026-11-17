// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Keeping asset folders in step with their notes (`reconcile`).
//!
//! A folder is named like its note's file, so a retitle leaves it stale: the
//! note has moved to a new slug and the folder keeps the old one until the
//! next `reconcile`. This step renames each folder to its note's current stem
//! and rewrites the links into it.
//!
//! Both halves go by the ULID, never by a remembered old name. A crash
//! between the rename and the rewrite leaves links to a folder that is
//! gone, and the next run finds them the same way it found the stale
//! folder: the link names a folder some note owns under another name, and
//! the folder that note owns now carries the stem. The same repair mends the
//! drift of a folder that was never renamed along with its note.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::note::Note;
use crate::vault::layout::{ALL_NOTES_DIR, Layout};

use super::{Place, local_links, owner, place, relink_folder};

/// A folder renamed, or a rename that was due and refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Links into a renamed folder that were rewritten in one note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relink {
    /// The note whose body changed.
    pub note: PathBuf,
    /// The folder name the links used.
    pub from: String,
    /// The folder name they use now.
    pub to: String,
    /// How many places in the body were rewritten.
    pub count: usize,
}

/// What [`sync`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AssetSync {
    /// Folders renamed to their note's current stem.
    pub renamed: Vec<FolderMove>,
    /// Renames not made because a folder of that name exists or the note owns
    /// several folders; each folder kept its name.
    pub refused: Vec<FolderMove>,
    /// Bodies rewritten in memory, one entry per note and folder name. The
    /// caller writes the notes.
    pub relinked: Vec<Relink>,
}

/// Folder names as `(from, to)` pairs.
type NamePairs = Vec<(String, String)>;

/// The names of the real directories in `_assets/`, sorted.
///
/// Hidden entries are left out: the Finder drops a `.DS_Store` into any
/// folder it opens. An `_assets` that is itself a symlink holds no folders
/// ntropy manages, and neither does a symlinked entry.
pub fn read_folders(layout: &Layout) -> Result<Vec<String>> {
    let assets = layout.assets_dir();
    if std::fs::symlink_metadata(&assets).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = fsutil::read_dir_entries(&assets)?
        .into_iter()
        .filter(|(_, file_type)| file_type.is_dir())
        .filter_map(|(path, _)| path.file_name()?.to_str().map(str::to_owned))
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    Ok(names)
}

/// The renames that bring each folder to its note's stem, and the ones that
/// cannot be made.
///
/// `stems` maps a note identity to the stem of its file. A folder whose
/// owner is no note in `stems` is not touched. When the folder with the exact
/// stem exists it is the note's folder and any other folder of that owner is
/// refused. Otherwise a single folder is renamed, and several are all
/// refused, since nothing says which one is the note's.
pub fn plan_renames(folders: &[String], stems: &HashMap<Id, String>) -> (NamePairs, NamePairs) {
    let mut by_owner: BTreeMap<Id, Vec<&String>> = BTreeMap::new();
    for folder in folders {
        if let Some(id) = owner(folder).filter(|id| stems.contains_key(id)) {
            by_owner.entry(id).or_default().push(folder);
        }
    }

    let mut renames = Vec::new();
    let mut refused = Vec::new();
    for (id, owned) in by_owner {
        let stem = &stems[&id];
        let named_right = owned.contains(&stem);
        match (named_right, owned.as_slice()) {
            (true, _) => {
                refused.extend(
                    owned
                        .iter()
                        .filter(|folder| **folder != stem)
                        .map(|folder| ((*folder).clone(), stem.clone())),
                );
            }
            (false, [only]) => renames.push(((*only).clone(), stem.clone())),
            (false, several) => {
                refused.extend(
                    several
                        .iter()
                        .map(|folder| ((*folder).clone(), stem.clone())),
                );
            }
        }
    }
    (renames, refused)
}

/// The folder names that links should be rewritten from, each with the stem
/// to rewrite to.
///
/// `listed` is the directory listing after the renames. A name qualifies
/// when a note owns it, the stem of that note differs from it byte for byte,
/// the listing holds the stem and does not hold the name. The listing is
/// compared and not the filesystem asked, since a case-insensitive
/// filesystem finds the old spelling of a folder that was renamed only in
/// case.
pub fn stale_names<'a>(
    linked: impl IntoIterator<Item = &'a str>,
    listed: &[String],
    stems: &HashMap<Id, String>,
) -> BTreeMap<String, String> {
    let listed: BTreeSet<&str> = listed.iter().map(String::as_str).collect();
    linked
        .into_iter()
        .filter_map(|name| {
            let stem = stems.get(&owner(name)?)?;
            (name != stem.as_str() && listed.contains(stem.as_str()) && !listed.contains(name))
                .then(|| (name.to_owned(), stem.clone()))
        })
        .collect()
}

/// Rename the folders of `notes` to their notes' stems and rewrite the links
/// into them in every note body.
///
/// Rewritten bodies are assigned to `Note::body` and nothing is written:
/// `reconcile` writes each note once, after its own rewrite of note links,
/// so the second pass cannot undo the first. The paths of `notes` must be
/// their final ones.
pub fn sync(layout: &Layout, notes: &mut [Note]) -> Result<AssetSync> {
    let mut report = AssetSync::default();
    let stems: HashMap<Id, String> = notes
        .iter()
        .filter_map(|note| {
            let stem = note.path.file_stem()?.to_str()?.to_owned();
            Some((note.id, stem))
        })
        .collect();

    let mut folders = read_folders(layout)?;
    let (renames, refused) = plan_renames(&folders, &stems);
    let moved = |(from, to): (String, String)| FolderMove {
        from: layout.assets_dir().join(from),
        to: layout.assets_dir().join(to),
    };
    for pair in renames {
        let change = moved(pair);
        match fsutil::rename(&change.from, &change.to) {
            Ok(()) => report.renamed.push(change),
            Err(error) if error.is_destination_taken() => report.refused.push(change),
            Err(error) => return Err(error.into()),
        }
    }
    report.refused.extend(refused.into_iter().map(moved));
    if !report.renamed.is_empty() {
        folders = read_folders(layout)?;
    }

    let note_dir = format!("/{ALL_NOTES_DIR}");
    for note in notes.iter_mut() {
        let linked: Vec<String> = local_links(&note.body)
            .iter()
            .filter_map(|link| match place(&note_dir, link.dest.as_str()) {
                Place::Managed { folder, .. } => Some(folder),
                _ => None,
            })
            .collect();
        let stale = stale_names(linked.iter().map(String::as_str), &folders, &stems);
        for (from, to) in stale {
            if let Some((body, count)) = relink_folder(&note.body, &from, &to) {
                note.body = body;
                report.relinked.push(Relink {
                    note: note.path.clone(),
                    from,
                    to,
                    count,
                });
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn id(text: &str) -> Id {
        text.parse().expect("a valid ULID")
    }

    fn stems(pairs: &[(&str, &str)]) -> HashMap<Id, String> {
        pairs
            .iter()
            .map(|(ulid, stem)| (id(ulid), (*stem).to_string()))
            .collect()
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
            .collect()
    }

    #[test]
    fn a_stale_folder_is_renamed_to_the_stem() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let (renames, refused) = plan_renames(&names(&[&format!("{ULID}-old")]), &stems);
        assert_eq!(
            renames,
            pairs(&[(&format!("{ULID}-old"), &format!("{ULID}-new"))])
        );
        assert!(refused.is_empty());
    }

    #[test]
    fn a_folder_named_like_the_stem_stays() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let (renames, refused) = plan_renames(&names(&[&format!("{ULID}-new")]), &stems);
        assert!(renames.is_empty() && refused.is_empty());
    }

    #[test]
    fn a_folder_that_differs_only_in_case_is_renamed() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let (renames, _) = plan_renames(&names(&[&format!("{ULID}-New")]), &stems);
        assert_eq!(
            renames,
            pairs(&[(&format!("{ULID}-New"), &format!("{ULID}-new"))])
        );
    }

    #[test]
    fn the_folder_with_the_stem_wins_over_a_stale_one() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new"), &format!("{ULID}-old")]);
        let (renames, refused) = plan_renames(&listed, &stems);
        assert!(renames.is_empty());
        assert_eq!(
            refused,
            pairs(&[(&format!("{ULID}-old"), &format!("{ULID}-new"))])
        );
    }

    #[test]
    fn two_stale_folders_are_all_refused() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-a"), &format!("{ULID}-b")]);
        let (renames, refused) = plan_renames(&listed, &stems);
        assert!(renames.is_empty());
        assert_eq!(refused.len(), 2);
    }

    #[test]
    fn a_folder_no_note_owns_is_left_alone() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{OTHER}-gone"), "shared"]);
        let (renames, refused) = plan_renames(&listed, &stems);
        assert!(renames.is_empty() && refused.is_empty());
    }

    #[test]
    fn a_link_to_a_folder_that_moved_is_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new")]);
        let stale = stale_names([format!("{ULID}-old").as_str()], &listed, &stems);
        assert_eq!(
            stale.into_iter().collect::<Vec<_>>(),
            pairs(&[(&format!("{ULID}-old"), &format!("{ULID}-new"))])
        );
    }

    #[test]
    fn a_link_to_a_folder_that_still_exists_is_not_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new"), &format!("{ULID}-old")]);
        let stale = stale_names([format!("{ULID}-old").as_str()], &listed, &stems);
        assert!(stale.is_empty());
    }

    #[test]
    fn a_link_to_a_note_whose_folder_does_not_exist_is_not_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let stale = stale_names([format!("{ULID}-old").as_str()], &[], &stems);
        assert!(stale.is_empty());
    }

    #[test]
    fn a_link_to_a_folder_no_note_owns_is_not_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new")]);
        let stale = stale_names(["shared", format!("{OTHER}-x").as_str()], &listed, &stems);
        assert!(stale.is_empty());
    }

    #[test]
    fn the_old_spelling_of_a_folder_renamed_in_case_is_stale_by_the_listing() {
        // A case-insensitive filesystem would find `-New` on disk after the
        // folder became `-new`; the listing holds only the new spelling.
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new")]);
        let stale = stale_names([format!("{ULID}-New").as_str()], &listed, &stems);
        assert_eq!(stale.len(), 1);
    }
}
