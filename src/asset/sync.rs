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
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::fsutil::{self, FsError};
use crate::id::Id;
use crate::note::Note;
use crate::vault::layout::Layout;

use super::{owner, references, relink_folder};

/// A folder renamed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Why a rename that was due was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// The note owns several folders, and nothing says which is its own.
    SeveralFolders,
    /// A folder of the new name exists.
    TargetExists,
    /// Something that is not a folder has the new name.
    TargetNotAFolder,
}

impl RefusalReason {
    /// The reason as a clause of the warning.
    pub fn text(self) -> &'static str {
        match self {
            RefusalReason::SeveralFolders => "the note has several asset folders",
            RefusalReason::TargetExists => "a folder of that name already exists",
            RefusalReason::TargetNotAFolder => "a file of that name is in the way",
        }
    }
}

/// A rename that was due and refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedMove {
    pub from: PathBuf,
    pub to: PathBuf,
    pub reason: RefusalReason,
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
    pub refused: Vec<RefusedMove>,
    /// Bodies rewritten in memory, one entry per note and folder name. The
    /// caller writes the notes.
    pub relinked: Vec<Relink>,
}

/// The first file or symlink below `assets_dir` that is not operating-system
/// metadata, in path order, or `None` when there is none.
///
/// Such a file would stay plaintext beside encrypted notes, which is what
/// `vault encrypt` refuses (asset encryption is not supported yet). Empty
/// folders hold nothing and pass. The names of OS metadata, which the Finder or
/// Explorer drop into any folder they open, pass too: `.DS_Store`, `._*`,
/// `Thumbs.db` and `desktop.ini`. Any other hidden file, such as `.env`, blocks.
pub fn first_unencryptable(assets_dir: &Path) -> std::result::Result<Option<PathBuf>, FsError> {
    let mut entries = fsutil::read_dir_entries(assets_dir)?;
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (path, file_type) in entries {
        // A symlink to a directory is no directory here: it is not followed.
        if file_type.is_dir() {
            if let Some(found) = first_unencryptable(&path)? {
                return Ok(Some(found));
            }
        } else if !is_system_metadata(&path) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Whether `path` is a file the Finder or Explorer drops into a folder.
fn is_system_metadata(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    matches!(name.as_str(), ".DS_Store" | "Thumbs.db" | "desktop.ini") || name.starts_with("._")
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

/// The names of every entry of `_assets/`, of any type, hidden ones included.
fn read_entries(layout: &Layout) -> Result<BTreeSet<String>> {
    let assets = layout.assets_dir();
    if std::fs::symlink_metadata(&assets).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Ok(BTreeSet::new());
    }
    Ok(fsutil::read_dir_entries(&assets)?
        .into_iter()
        .filter_map(|(path, _)| path.file_name()?.to_str().map(str::to_owned))
        .collect())
}

/// The folder names that links should be rewritten from, each with the stem
/// to rewrite to.
///
/// `folders` and `entries` are the listing of `_assets/` after the renames: its
/// directories, and every entry of any type. A name qualifies when a note owns
/// it, the stem of that note differs from it byte for byte, the folders hold
/// the stem, and no entry of any type has the name. A file or a symlink of that
/// name is no stale folder: the link may name it. The listing is compared and
/// not the filesystem asked, since a case-insensitive filesystem finds the old
/// spelling of a folder that was renamed only in case.
pub fn stale_names<'a>(
    linked: impl IntoIterator<Item = &'a str>,
    folders: &[String],
    entries: &BTreeSet<String>,
    stems: &HashMap<Id, String>,
) -> BTreeMap<String, String> {
    let folders: BTreeSet<&str> = folders.iter().map(String::as_str).collect();
    linked
        .into_iter()
        .filter_map(|name| {
            let stem = stems.get(&owner(name)?)?;
            (name != stem.as_str() && folders.contains(stem.as_str()) && !entries.contains(name))
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
            Err(error) if error.is_destination_taken() => {
                let target_is_dir = std::fs::symlink_metadata(&change.to)
                    .is_ok_and(|metadata| metadata.file_type().is_dir());
                report.refused.push(RefusedMove {
                    from: change.from,
                    to: change.to,
                    reason: if target_is_dir {
                        RefusalReason::TargetExists
                    } else {
                        RefusalReason::TargetNotAFolder
                    },
                });
            }
            Err(error) => return Err(error.into()),
        }
    }
    report
        .refused
        .extend(refused.into_iter().map(moved).map(|change| RefusedMove {
            from: change.from,
            to: change.to,
            reason: RefusalReason::SeveralFolders,
        }));
    if !report.renamed.is_empty() {
        folders = read_folders(layout)?;
    }
    let entries = read_entries(layout)?;

    for note in notes.iter_mut() {
        let referenced: Vec<String> = references(&note.body)
            .into_iter()
            .filter(|reference| reference.slash)
            .map(|reference| reference.name)
            .collect();
        let stale = stale_names(
            referenced.iter().map(String::as_str),
            &folders,
            &entries,
            &stems,
        );
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

    fn entries_of(names: &[String]) -> BTreeSet<String> {
        names.iter().cloned().collect()
    }

    #[test]
    fn a_loose_file_with_the_old_name_is_no_stale_folder() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let folders = names(&[&format!("{ULID}-new")]);
        let mut entries = entries_of(&folders);
        entries.insert(format!("{ULID}-notes.txt"));

        let stale = stale_names(
            [format!("{ULID}-notes.txt").as_str()],
            &folders,
            &entries,
            &stems,
        );

        assert!(stale.is_empty());
    }

    fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
            .collect()
    }

    fn assets_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        for file in files {
            let path = dir.path().join(file);
            if let Some(name) = file.strip_suffix('/') {
                std::fs::create_dir_all(dir.path().join(name)).expect("directory");
            } else {
                std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
                std::fs::write(path, b"x").expect("file");
            }
        }
        dir
    }

    #[test]
    fn a_file_in_a_folder_blocks_encryption() {
        let dir = assets_with(&["folder/chart.png"]);
        assert_eq!(
            first_unencryptable(dir.path()).expect("walk"),
            Some(dir.path().join("folder/chart.png"))
        );
    }

    #[test]
    fn a_file_directly_in_the_assets_directory_blocks_encryption() {
        let dir = assets_with(&["loose.png"]);
        assert!(first_unencryptable(dir.path()).expect("walk").is_some());
    }

    #[test]
    fn the_first_file_in_path_order_is_named() {
        let dir = assets_with(&["b/two.png", "a/one.png", "a/zero.png"]);
        assert_eq!(
            first_unencryptable(dir.path()).expect("walk"),
            Some(dir.path().join("a/one.png"))
        );
    }

    #[test]
    fn empty_folders_pass() {
        let dir = assets_with(&["a/", "b/c/d/"]);
        assert_eq!(first_unencryptable(dir.path()).expect("walk"), None);
    }

    #[test]
    fn a_missing_assets_directory_passes() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            first_unencryptable(&dir.path().join("no-such-dir")).expect("walk"),
            None
        );
    }

    #[test]
    fn operating_system_metadata_passes() {
        let dir = assets_with(&[
            ".DS_Store",
            "folder/.DS_Store",
            "folder/._chart.png",
            "folder/Thumbs.db",
            "folder/desktop.ini",
        ]);
        assert_eq!(first_unencryptable(dir.path()).expect("walk"), None);
    }

    #[test]
    fn another_hidden_file_blocks_encryption() {
        let dir = assets_with(&["folder/.env"]);
        assert_eq!(
            first_unencryptable(dir.path()).expect("walk"),
            Some(dir.path().join("folder/.env"))
        );
    }

    #[test]
    fn a_symlink_blocks_encryption() {
        let dir = assets_with(&["folder/"]);
        std::os::unix::fs::symlink("/", dir.path().join("folder/link")).expect("symlink");
        assert_eq!(
            first_unencryptable(dir.path()).expect("walk"),
            Some(dir.path().join("folder/link"))
        );
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
        let stale = stale_names(
            [format!("{ULID}-old").as_str()],
            &listed,
            &entries_of(&listed),
            &stems,
        );
        assert_eq!(
            stale.into_iter().collect::<Vec<_>>(),
            pairs(&[(&format!("{ULID}-old"), &format!("{ULID}-new"))])
        );
    }

    #[test]
    fn a_link_to_a_folder_that_still_exists_is_not_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new"), &format!("{ULID}-old")]);
        let stale = stale_names(
            [format!("{ULID}-old").as_str()],
            &listed,
            &entries_of(&listed),
            &stems,
        );
        assert!(stale.is_empty());
    }

    #[test]
    fn a_link_to_a_note_whose_folder_does_not_exist_is_not_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let stale = stale_names(
            [format!("{ULID}-old").as_str()],
            &[],
            &BTreeSet::new(),
            &stems,
        );
        assert!(stale.is_empty());
    }

    #[test]
    fn a_link_to_a_folder_no_note_owns_is_not_stale() {
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new")]);
        let stale = stale_names(
            ["shared", format!("{OTHER}-x").as_str()],
            &listed,
            &entries_of(&listed),
            &stems,
        );
        assert!(stale.is_empty());
    }

    #[test]
    fn the_old_spelling_of_a_folder_renamed_in_case_is_stale_by_the_listing() {
        // A case-insensitive filesystem would find `-New` on disk after the
        // folder became `-new`; the listing holds only the new spelling.
        let stems = stems(&[(ULID, &format!("{ULID}-new"))]);
        let listed = names(&[&format!("{ULID}-new")]);
        let stale = stale_names(
            [format!("{ULID}-New").as_str()],
            &listed,
            &entries_of(&listed),
            &stems,
        );
        assert_eq!(stale.len(), 1);
    }
}
