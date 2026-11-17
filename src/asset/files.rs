// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Putting files into a note's asset folder and listing what is there
//! (`ntropy asset add` and `ntropy asset list`, ADR 0104).
//!
//! The folder is found by the ULID of the note, so between a retitle and the
//! next `reconcile` files land in the folder under its old name, and the link
//! this module builds carries that name. Nothing here edits a note: the link
//! is returned for the caller to print.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::fsutil::{self, FsError};
use crate::id::Id;
use crate::note::Note;
use crate::vault::layout::{ALL_NOTES_DIR, ASSETS_DIR, Layout};

use super::{
    FolderChoice, Place, choose_folder, html_ranges, is_system_metadata, local_links, owner, place,
    read_folders,
};

/// Why an asset could not be added or listed.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error(transparent)]
    Vault(#[from] crate::error::Error),

    #[error(transparent)]
    Fs(#[from] FsError),

    #[error("`{}` is not a file", .0.display())]
    NotAFile(PathBuf),

    #[error(
        "`{name}` cannot be stored as it is: {reason}; pass --as <name> to store it under \
         another name"
    )]
    BadName { name: String, reason: &'static str },

    #[error("`{0}` already exists in the note's folder; pass --as <name> or --force")]
    Exists(String),

    #[error(
        "`{}` is already the file `{}` in the note's folder; nothing was changed",
        given.display(),
        target.display()
    )]
    SameFile { given: PathBuf, target: PathBuf },

    #[error(
        "`{first}` differs only in case from `{second}`; names that differ only in case are one \
         file on some filesystems"
    )]
    CaseClash { first: String, second: String },

    #[error(
        "`{}` is a symlink, which ntropy leaves alone, so no assets can be added to it or \
         listed from it",
        .0.display()
    )]
    Symlinked(PathBuf),

    #[error("could not store `{name}`: {cause}")]
    Transfer {
        name: String,
        #[source]
        cause: FsError,
    },

    #[error(
        "stored `{name}`, but could not remove the source `{}`: {cause}; the file is now in \
         both places",
        given.display()
    )]
    SourceKept {
        name: String,
        given: PathBuf,
        #[source]
        cause: FsError,
    },

    #[error("`{}` is a note of this vault", .0.display())]
    NoteFile(PathBuf),

    #[error("`--as` names one file, but {0} files were given")]
    RenameNeedsOneFile(usize),

    #[error("two of the files would be stored as `{0}`; pass --as <name> for one of them")]
    StoredTwice(String),

    #[error(
        "the note has several asset folders ({}); run `ntropy reconcile`, or merge them first",
        .0.join(", ")
    )]
    SeveralFolders(Vec<String>),
}

/// What `asset add` was asked to do with each file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddOptions {
    /// Move the files instead of copying them.
    pub move_files: bool,
    /// Store the one file under this name.
    pub rename: Option<String>,
    /// Replace a file of the same name.
    pub force: bool,
}

/// What `asset add` did: the files it placed, whether it touched the disk,
/// and the error that stopped it, if any.
#[derive(Debug)]
pub struct AddReport {
    /// The files placed, in order. Also set when a later file failed.
    pub added: Vec<Added>,
    /// Whether the folder was created or a file placed, so the views may need
    /// to follow.
    pub touched: bool,
    /// What the caller should warn about: the run went on regardless.
    pub warnings: Vec<String>,
    /// Why the command stopped before the last file.
    pub error: Option<AssetError>,
}

/// A file put into the note's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// Where the file is now.
    pub path: PathBuf,
    /// The Markdown link to put into the note.
    pub link: String,
}

/// The folder of `note`: the one it owns under whatever name it has, or the
/// one named like the note when it has none yet.
///
/// A symlinked `_assets` directory, or a symlink where the note's folder
/// belongs, is refused: ntropy leaves such an entry alone (ADR 0098), and a
/// file added or listed through it would belong to another note or to a place
/// outside the vault, under a link that names this note's folder.
pub fn folder_of(layout: &Layout, note: &Note) -> Result<PathBuf, AssetError> {
    let is_symlink = |path: &Path| {
        std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
    };
    if is_symlink(&layout.assets_dir()) {
        return Err(AssetError::Symlinked(layout.assets_dir()));
    }
    let stem = note
        .path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let listed = read_folders(layout)?;
    let name = match choose_folder(&listed, note.id, &stem) {
        FolderChoice::Existing(name) | FolderChoice::Create(name) => name,
        FolderChoice::Several(names) => return Err(AssetError::SeveralFolders(names)),
    };
    let folder = layout.asset_folder(&name);
    if is_symlink(&folder) {
        return Err(AssetError::Symlinked(folder));
    }
    Ok(folder)
}

/// File extensions whose link is an image, `![..](..)`.
const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp"];

/// Whether the file `name` has an image extension, ignoring case.
pub fn is_image_name(name: &str) -> bool {
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    IMAGE_EXTENSIONS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(extension))
}

/// The Markdown link to the file `name` in the folder `folder`: an image for
/// an image extension, a link otherwise, with the target in angle brackets
/// when the name holds anything but letters, digits, `.`, `_` and `-`.
///
/// The link is read back with the parser the renderers use, and a name whose
/// link would not read back as this file, one with an entity such as `&amp;`
/// in it, is refused.
fn link_for(folder: &str, name: &str) -> Result<String, AssetError> {
    let refuse = |reason| AssetError::BadName {
        name: name.to_owned(),
        reason,
    };
    if name.is_empty() || name == "." || name == ".." {
        return Err(refuse("it is not a file name"));
    }
    if name.chars().any(char::is_control) {
        return Err(refuse("it holds a control character"));
    }
    if name.contains(['/', '\\', '#', '<', '>']) {
        return Err(refuse("it holds a character no link can carry"));
    }

    let dest = format!("{ASSETS_DIR}/{folder}/{name}");
    let plain = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let target = if plain {
        dest.clone()
    } else {
        format!("<{dest}>")
    };

    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let image = is_image_name(name);
    let label = if image && !stem.is_empty() {
        stem
    } else {
        name
    };
    let label: String = label
        .chars()
        .flat_map(|c| match c {
            '\\' | '[' | ']' => vec!['\\', c],
            c => vec![c],
        })
        .collect();
    let bang = if image { "!" } else { "" };
    let link = format!("{bang}[{label}]({target})");

    let reads_back = local_links(&link)
        .first()
        .is_some_and(|read| read.dest == dest);
    if reads_back {
        Ok(link)
    } else {
        Err(refuse("its link would not read back as this file"))
    }
}

/// Copy, or with `options.move_files` move, `files` into the folder of
/// `note`, creating it when needed.
///
/// Everything is checked before the first file is touched: that each source is
/// a file, that it is not already the file it would be stored as, that each
/// name can be linked, that no two names differ only in case, from each other
/// or from a file of the folder, and, without `force`, that no file of that
/// name is there. An I/O failure during the transfer cannot be checked ahead:
/// the files placed before it stay, and the report names them together with
/// the error, so the caller can show what happened.
///
/// A symlink as a source is followed for a copy. A move of one copies what it
/// points to and then removes the link itself, never its target, since moving
/// the link would put a symlink into the folder.
pub fn add(layout: &Layout, note: &Note, files: &[PathBuf], options: &AddOptions) -> AddReport {
    let mut report = AddReport {
        added: Vec::new(),
        touched: false,
        warnings: Vec::new(),
        error: None,
    };
    let Plan {
        folder,
        files: planned,
        warnings,
    } = match plan(layout, note, files, options) {
        Ok(plan) => plan,
        Err(error) => {
            report.error = Some(error);
            return report;
        }
    };
    report.warnings = warnings;
    if let Err(error) = fsutil::create_dir_all(&folder) {
        report.error = Some(error.into());
        return report;
    }
    report.touched = true;

    let mut placed: BTreeMap<Identity, String> = BTreeMap::new();
    for (source, name, link) in planned {
        let path = folder.join(&name);
        // A name the checks told apart can still be one entry on this
        // filesystem. A forced placement would replace the file this run has
        // just stored under the other spelling, so it stops instead.
        if let Some(first) = placed_earlier(&placed, &path) {
            report.error = Some(AssetError::CaseClash {
                first,
                second: name,
            });
            return report;
        }
        let outcome = transfer(&source, &path, &folder, options);
        if let Some(identity) = identity_of(&path).filter(|_| outcome.stored()) {
            placed.insert(identity, name.clone());
        }
        match outcome {
            Transfer::Done => report.added.push(Added { path, link }),
            Transfer::NotStored(cause) => {
                report.error = Some(AssetError::Transfer { name, cause });
                return report;
            }
            Transfer::SourceKept(cause) => {
                report.added.push(Added { path, link });
                report.error = Some(AssetError::SourceKept {
                    name,
                    given: source,
                    cause,
                });
                return report;
            }
        }
    }
    report
}

/// Per file, the source, the name it is stored under and its link.
type Planned = Vec<(PathBuf, String, String)>;

/// What `add` will do, after every check that can run before a file is
/// touched.
struct Plan {
    folder: PathBuf,
    files: Planned,
    warnings: Vec<String>,
}

fn plan(
    layout: &Layout,
    note: &Note,
    files: &[PathBuf],
    options: &AddOptions,
) -> Result<Plan, AssetError> {
    if options.rename.is_some() && files.len() != 1 {
        return Err(AssetError::RenameNeedsOneFile(files.len()));
    }
    let folder = folder_of(layout, note)?;
    let folder_name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    // The entries a name may fold onto, read once: a file of this folder is
    // replaced by `--force` only under exactly its own spelling.
    let present: Vec<String> = fsutil::read_dir_entries(&folder)?
        .into_iter()
        .filter_map(|(path, _)| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();

    let mut planned: Planned = Vec::new();
    let mut warnings = Vec::new();
    for source in files {
        if !std::fs::metadata(source).is_ok_and(|metadata| metadata.is_file()) {
            return Err(AssetError::NotAFile(source.clone()));
        }
        if is_a_note_file(layout, source) {
            return Err(AssetError::NoteFile(source.clone()));
        }
        if options.move_files
            && let Some(warning) = leaves_another_notes_folder(layout, note.id, source)
        {
            warnings.push(warning);
        }
        let name = match &options.rename {
            Some(name) => name.clone(),
            None => match source.file_name().and_then(|name| name.to_str()) {
                Some(name) => name.to_owned(),
                None => {
                    return Err(AssetError::BadName {
                        name: source.display().to_string(),
                        reason: "it is not valid UTF-8",
                    });
                }
            },
        };
        let link = link_for(&folder_name, &name)?;
        let target = folder.join(&name);
        if is_the_target(source, &target) {
            return Err(AssetError::SameFile {
                given: source.clone(),
                target,
            });
        }
        if planned.iter().any(|(_, taken, _)| *taken == name) {
            return Err(AssetError::StoredTwice(name));
        }
        let folded = fold(&name);
        let clashing = planned
            .iter()
            .map(|(_, taken, _)| taken)
            .chain(present.iter())
            .find(|other| **other != name && fold(other) == folded);
        if let Some(other) = clashing {
            return Err(AssetError::CaseClash {
                first: other.clone(),
                second: name,
            });
        }
        if !options.force && std::fs::symlink_metadata(folder.join(&name)).is_ok() {
            return Err(AssetError::Exists(name));
        }
        planned.push((source.clone(), name, link));
    }
    Ok(Plan {
        folder,
        files: planned,
        warnings,
    })
}

/// The name as a filesystem that folds case would compare it.
///
/// Every character goes to upper case and back to lower case, so `ſ` and `s`,
/// `µ` and `μ`, `ς` and `σ`, and `ß` and `ss` meet, which `str::to_lowercase`
/// misses: it depends on the context of a final sigma and leaves the rest
/// alone. That is a superset of simple case folding. Canonically equivalent
/// spellings (an accent as one character or as two) are not folded here; the
/// check at transfer time catches them on the filesystems that treat them as
/// one name.
fn fold(name: &str) -> String {
    name.chars()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}

/// The entry identity of a file this run placed: device and inode.
type Identity = (u64, u64);

/// The entry identity of the directory entry at `path`, not following a symlink.
fn identity_of(path: &Path) -> Option<Identity> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    Some((metadata.dev(), metadata.ino()))
}

/// The name of the file placed earlier in this run that `target` is, if any.
///
/// Whatever the filesystem folds or normalizes, two names that are one entry
/// have one identity, so a forced placement can refuse to replace a file this
/// run has just stored.
fn placed_earlier(placed: &BTreeMap<Identity, String>, target: &Path) -> Option<String> {
    placed.get(&identity_of(target)?).cloned()
}

/// The directory entry `source` names, with the directories on the way
/// resolved and the last component left as it is: the entry a move removes,
/// which for a symlink is the link and not what it points to.
fn entry_path(source: &Path) -> Option<PathBuf> {
    let parent = source
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Some(
        std::fs::canonicalize(parent)
            .ok()?
            .join(source.file_name()?),
    )
}

/// Whether `source` is a note of the vault: a `.md` or `.age` file directly in
/// `all-notes/`.
///
/// It is judged by the entry the path names (the directories resolved, so a
/// different spelling of `all-notes/` or a symlinked directory on the way is
/// seen) and by the file it finally reaches (so a symlink standing for a note,
/// such as a leaf of a view, is seen). A move of a note would take it out of
/// the vault's notes, a copy would put its text into an asset folder.
fn is_a_note_file(layout: &Layout, source: &Path) -> bool {
    let Ok(notes) = std::fs::canonicalize(layout.all_notes()) else {
        return false;
    };
    let is_note = |path: &Path| {
        path.parent() == Some(notes.as_path())
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("age")
                })
    };
    entry_path(source).is_some_and(|entry| is_note(&entry))
        || std::fs::canonicalize(source).is_ok_and(|reached| is_note(&reached))
}

/// The warning for a move that takes `source` out of the asset folder of
/// another note, whose links to it break, `None` when it does not.
///
/// The entry a move removes is judged, so a symlink standing in another note's
/// folder counts and a symlink elsewhere that points into it does not. A folder
/// no note owns holds no links, so it gets no warning.
fn leaves_another_notes_folder(layout: &Layout, note: Id, source: &Path) -> Option<String> {
    let assets = std::fs::canonicalize(layout.assets_dir()).ok()?;
    let entry = entry_path(source)?;
    let folder = entry
        .strip_prefix(&assets)
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_str()?
        .to_owned();
    let owner = owner(&folder).filter(|owner| *owner != note)?;
    let prefix = owner.to_string();
    let owner_exists = fsutil::read_dir_entries(&layout.all_notes())
        .ok()?
        .into_iter()
        .any(|(path, _)| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(&prefix) && (name.ends_with(".md") || name.ends_with(".age"))
                })
        });
    owner_exists.then(|| {
        format!(
            "moving `{}` out of the asset folder `{folder}` of another note breaks the links \
             that note holds to it",
            source.display()
        )
    })
}

/// Whether `source` is the entry at `target` or a way to reach its file.
///
/// A move or a forced copy puts the file at `target` and then removes the
/// source, so when both are one directory entry the removal deletes the only
/// copy. The entries are compared by device and inode, which sees through a
/// symlinked directory on the way (a view's `_assets` link), a hard link and
/// the spelling of a name on a filesystem that folds case. A source that is a
/// symlink to the target counts as well, since the copy would read the file it
/// is about to replace. A symlink standing at `target` that points at the source
/// is a different entry and only the link is replaced.
fn is_the_target(source: &Path, target: &Path) -> bool {
    let identity = |metadata: std::io::Result<std::fs::Metadata>| {
        metadata
            .ok()
            .map(|metadata| (metadata.dev(), metadata.ino()))
    };
    let Some(at_target) = identity(std::fs::symlink_metadata(target)) else {
        return false;
    };
    identity(std::fs::symlink_metadata(source)) == Some(at_target)
        || identity(std::fs::metadata(source)) == Some(at_target)
}

/// How placing one file ended.
enum Transfer {
    Done,
    /// The file was not stored.
    NotStored(FsError),
    /// The file was stored and the source could not be removed.
    SourceKept(FsError),
}

impl Transfer {
    /// Whether a file now stands at the target.
    fn stored(&self) -> bool {
        !matches!(self, Transfer::NotStored(_))
    }
}

/// Put `source` at `target`, inside `folder`.
fn transfer(source: &Path, target: &Path, folder: &Path, options: &AddOptions) -> Transfer {
    let stored = |result: Result<(), FsError>| match result {
        Ok(()) => Transfer::Done,
        Err(cause) => Transfer::NotStored(cause),
    };
    if !options.move_files {
        return stored(fsutil::copy_file(source, target, options.force));
    }
    // A rename keeps one file on one device, and moves a symlink as the link,
    // so only a regular file on the folder's own device is renamed.
    let is_link = std::fs::symlink_metadata(source).is_ok_and(|m| m.file_type().is_symlink());
    let same_device = matches!(
        (std::fs::metadata(source), std::fs::metadata(folder)),
        (Ok(a), Ok(b)) if a.dev() == b.dev()
    );
    if !is_link && same_device && !options.force {
        return stored(fsutil::rename(source, target));
    }
    if let Err(cause) = fsutil::copy_file(source, target, options.force) {
        return Transfer::NotStored(cause);
    }
    match fsutil::remove_file(source) {
        Ok(()) => Transfer::Done,
        Err(cause) => Transfer::SourceKept(cause),
    }
}

/// Whether a file of the folder is linked by the note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linked {
    Yes,
    No,
    /// The note links it and it is not in the folder.
    Missing,
}

/// One row of `asset list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFile {
    /// The path below the folder, with `/` separators.
    pub name: String,
    /// The size in bytes, `None` for a file the note links and the folder
    /// lacks.
    pub size: Option<u64>,
    pub linked: Linked,
}

/// What a note's asset folder holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// The folder, whether it exists or not.
    pub folder: PathBuf,
    pub files: Vec<ListedFile>,
}

/// The files in the folder of `note`, operating-system metadata left out, and
/// whether the note links each. Hidden files are listed: `delete` counts and
/// removes them, so the listing names what a deletion would take. A file the note links in its own folder and the folder
/// lacks is listed too, as missing. A symlink is listed with its own size and
/// is never followed.
pub fn list(layout: &Layout, note: &Note) -> Result<Listing, AssetError> {
    let folder = folder_of(layout, note)?;

    let mut found: BTreeMap<String, u64> = BTreeMap::new();
    let mut pending = vec![(folder.clone(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        for (path, file_type) in fsutil::read_dir_entries(&dir)? {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if is_system_metadata(&path) {
                continue;
            }
            let relative = format!("{prefix}{name}");
            if file_type.is_dir() {
                pending.push((path, format!("{relative}/")));
            } else {
                let size = std::fs::symlink_metadata(&path).map_or(0, |metadata| metadata.len());
                found.insert(relative, size);
            }
        }
    }

    let note_dir = format!("/{ALL_NOTES_DIR}");
    let linked: BTreeSet<String> = local_links(&note.body)
        .iter()
        .filter_map(|link| match place(&note_dir, &link.dest) {
            Place::Managed { folder, rest } if owner(&folder) == Some(note.id) => {
                (!rest.is_empty()).then_some(rest)
            }
            _ => None,
        })
        .collect();

    // Raw HTML (`<img src=".." width="300">`, the usual way to size a picture)
    // uses a file as much as a link does. It only upgrades a file that is
    // there: whether a name it holds is a file is for a Markdown link to say,
    // since `reconcile` reports a missing file from those alone.
    let folder_name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let html: Vec<&str> = html_ranges(&note.body)
        .into_iter()
        .map(|range| &note.body[range])
        .collect();
    let used_in_html = |name: &str| {
        html.iter()
            .any(|slice| names_the_file(slice, &folder_name, name))
    };

    let mut files: Vec<ListedFile> = found
        .iter()
        .map(|(name, size)| ListedFile {
            name: name.clone(),
            size: Some(*size),
            linked: if linked.contains(name) || used_in_html(name) {
                Linked::Yes
            } else {
                Linked::No
            },
        })
        .collect();
    files.extend(
        linked
            .iter()
            .filter(|name| !found.contains_key(*name))
            .map(|name| ListedFile {
                name: name.clone(),
                size: None,
                linked: Linked::Missing,
            }),
    );
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Listing { folder, files })
}

/// Whether `text` holds `_assets/<folder>/<name>`, the `_assets` component
/// matched ignoring ASCII case, the way an HTML attribute names the file.
fn names_the_file(text: &str, folder: &str, name: &str) -> bool {
    let wanted = format!("{folder}/{name}");
    let marker = ASSETS_DIR.len() + 1;
    text.as_bytes()
        .windows(marker)
        .enumerate()
        .any(|(at, window)| {
            window[..ASSETS_DIR.len()].eq_ignore_ascii_case(ASSETS_DIR.as_bytes())
                && window[ASSETS_DIR.len()] == b'/'
                && text[at + marker..].starts_with(&wanted)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan;
    use crate::session::VaultSession;
    use crate::test_support::{vault_with_view, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const FOLDER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV-note";

    fn note_with(session: &VaultSession, body: &str) -> Note {
        write_note(
            session,
            &format!("{FOLDER}.md"),
            &format!("---\ntitle: Note\n---\n{body}\n"),
        );
        let mut scan =
            scan::scan_notes_dir(&session.layout().all_notes(), session.cipher()).expect("scan");
        scan.notes.pop().expect("the note")
    }

    fn source(session: &VaultSession, name: &str, content: &[u8]) -> PathBuf {
        let dir = session.root().join("incoming");
        std::fs::create_dir_all(&dir).expect("incoming");
        let path = dir.join(name);
        std::fs::write(&path, content).expect("source");
        path
    }

    fn opts() -> AddOptions {
        AddOptions::default()
    }

    /// `add` as a `Result`, for the tests that only care about the outcome.
    fn add(
        layout: &Layout,
        note: &Note,
        files: &[PathBuf],
        options: &AddOptions,
    ) -> Result<Vec<Added>, AssetError> {
        let report = super::add(layout, note, files, options);
        match report.error {
            Some(error) => Err(error),
            None => Ok(report.added),
        }
    }

    // ---- The link ----

    #[test]
    fn an_image_gets_a_bang_and_its_stem_as_alt_text() {
        assert_eq!(
            link_for(FOLDER, "chart.png").expect("link"),
            format!("![chart](_assets/{FOLDER}/chart.png)")
        );
    }

    #[test]
    fn any_other_file_gets_a_plain_link_named_like_the_file() {
        assert_eq!(
            link_for(FOLDER, "minutes.pdf").expect("link"),
            format!("[minutes.pdf](_assets/{FOLDER}/minutes.pdf)")
        );
    }

    #[test]
    fn the_image_extension_is_matched_ignoring_case() {
        assert!(
            link_for(FOLDER, "SHOT.PNG")
                .expect("link")
                .starts_with("![SHOT]")
        );
    }

    #[test]
    fn a_name_with_a_space_or_a_parenthesis_goes_in_angle_brackets() {
        assert_eq!(
            link_for(FOLDER, "my pic.png").expect("link"),
            format!("![my pic](<_assets/{FOLDER}/my pic.png>)")
        );
        assert_eq!(
            link_for(FOLDER, "scan (1).pdf").expect("link"),
            format!("[scan (1).pdf](<_assets/{FOLDER}/scan (1).pdf>)")
        );
    }

    #[test]
    fn brackets_in_the_label_are_escaped() {
        assert_eq!(
            link_for(FOLDER, "a[1].pdf").expect("link"),
            format!("[a\\[1\\].pdf](<_assets/{FOLDER}/a[1].pdf>)")
        );
    }

    #[test]
    fn names_no_link_can_carry_are_refused() {
        for name in [
            "a#b.png", "a\\b.png", "a<b>.png", "a/b.png", "a\nb.png", "", ".", "..",
        ] {
            assert!(
                matches!(link_for(FOLDER, name), Err(AssetError::BadName { .. })),
                "{name:?}"
            );
        }
    }

    #[test]
    fn a_name_whose_link_would_not_read_back_is_refused() {
        let err = link_for(FOLDER, "x&amp;y.pdf").expect_err("an entity");
        assert!(err.to_string().contains("would not read back"), "{err}");
    }

    #[test]
    fn a_name_with_a_lone_ampersand_is_fine() {
        assert!(link_for(FOLDER, "R&D.pdf").is_ok());
    }

    #[test]
    fn every_name_that_is_accepted_reads_back_as_the_file() {
        for name in [
            "a.png",
            "my pic.png",
            "scan (1).pdf",
            "R&D.pdf",
            "ü ö.png",
            "x%20y.png",
            "a'b.txt",
        ] {
            let link = link_for(FOLDER, name).expect(name);
            let read = local_links(&link);
            assert_eq!(read[0].dest, format!("_assets/{FOLDER}/{name}"), "{link}");
        }
    }

    // ---- add ----

    #[test]
    fn a_file_is_copied_into_the_notes_folder_and_the_link_is_returned() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let file = source(&session, "chart.png", b"png");

        let added = add(
            session.layout(),
            &note,
            std::slice::from_ref(&file),
            &opts(),
        )
        .expect("add");

        let stored = session.layout().asset_folder(FOLDER).join("chart.png");
        assert_eq!(std::fs::read(&stored).expect("read"), b"png");
        assert!(file.exists(), "a copy leaves the source");
        assert_eq!(
            added,
            [Added {
                path: stored,
                link: format!("![chart](_assets/{FOLDER}/chart.png)")
            }]
        );
    }

    #[test]
    fn several_files_are_added_in_order() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let files = [
            source(&session, "b.pdf", b"b"),
            source(&session, "a.png", b"a"),
        ];

        let added = add(session.layout(), &note, &files, &opts()).expect("add");

        assert_eq!(added.len(), 2);
        assert!(added[0].link.starts_with("[b.pdf]"));
        assert!(added[1].link.starts_with("![a]"));
    }

    #[test]
    fn as_stores_one_file_under_another_name() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let file = source(&session, "IMG_4711.png", b"png");
        let options = AddOptions {
            rename: Some("org-chart.png".to_string()),
            ..opts()
        };

        let added = add(session.layout(), &note, &[file], &options).expect("add");

        assert!(added[0].path.ends_with("org-chart.png"));
        assert!(added[0].link.starts_with("![org-chart]"));
    }

    #[test]
    fn as_with_two_files_is_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let files = [
            source(&session, "a.png", b"a"),
            source(&session, "b.png", b"b"),
        ];
        let options = AddOptions {
            rename: Some("x.png".to_string()),
            ..opts()
        };

        let err = add(session.layout(), &note, &files, &options).expect_err("two files");

        assert!(matches!(err, AssetError::RenameNeedsOneFile(2)));
    }

    #[test]
    fn an_existing_file_is_refused_and_nothing_is_copied() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("a.png"), b"old").expect("existing");
        let files = [
            source(&session, "b.png", b"b"),
            source(&session, "a.png", b"new"),
        ];

        let err = add(session.layout(), &note, &files, &opts()).expect_err("a conflict");

        assert!(matches!(err, AssetError::Exists(name) if name == "a.png"));
        assert_eq!(std::fs::read(folder.join("a.png")).expect("read"), b"old");
        assert!(!folder.join("b.png").exists(), "nothing was copied");
    }

    #[test]
    fn force_replaces_an_existing_file() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("a.png"), b"old").expect("existing");
        let options = AddOptions {
            force: true,
            ..opts()
        };

        add(
            session.layout(),
            &note,
            &[source(&session, "a.png", b"new")],
            &options,
        )
        .expect("add");

        assert_eq!(std::fs::read(folder.join("a.png")).expect("read"), b"new");
    }

    #[test]
    fn two_files_with_one_name_are_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let one = source(&session, "a.png", b"1");
        let two_dir = session.root().join("other");
        std::fs::create_dir_all(&two_dir).expect("dir");
        let two = two_dir.join("a.png");
        std::fs::write(&two, b"2").expect("file");

        let err = add(session.layout(), &note, &[one, two], &opts()).expect_err("twice");

        assert!(matches!(err, AssetError::StoredTwice(_)));
    }

    #[test]
    fn a_directory_and_a_missing_file_are_not_files() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        for path in [
            session.root().to_path_buf(),
            session.root().join("no-such-file"),
        ] {
            let err = add(session.layout(), &note, &[path], &opts()).expect_err("not a file");
            assert!(matches!(err, AssetError::NotAFile(_)));
        }
    }

    #[test]
    fn a_source_that_is_not_utf8_needs_as() {
        use std::os::unix::ffi::OsStrExt;
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let name = std::ffi::OsStr::from_bytes(b"caf\xe9.png");
        let path = session.root().join(name);
        // Some filesystems refuse such a name; there the case cannot arise.
        if std::fs::write(&path, b"x").is_err() {
            return;
        }

        let err = add(session.layout(), &note, &[path], &opts()).expect_err("not UTF-8");

        assert!(err.to_string().contains("not valid UTF-8"), "{err}");
    }

    #[test]
    fn a_move_takes_the_file_out_of_its_place() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let file = source(&session, "shot.png", b"png");
        let options = AddOptions {
            move_files: true,
            ..opts()
        };

        add(
            session.layout(),
            &note,
            std::slice::from_ref(&file),
            &options,
        )
        .expect("add");

        assert!(!file.exists());
        assert!(
            session
                .layout()
                .asset_folder(FOLDER)
                .join("shot.png")
                .is_file()
        );
    }

    #[test]
    fn a_move_of_a_symlink_stores_the_file_and_removes_the_link_only() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let target = source(&session, "real.png", b"png");
        let link = session.root().join("incoming/link.png");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        let options = AddOptions {
            move_files: true,
            ..opts()
        };

        add(
            session.layout(),
            &note,
            std::slice::from_ref(&link),
            &options,
        )
        .expect("add");

        let stored = session.layout().asset_folder(FOLDER).join("link.png");
        assert!(
            !std::fs::symlink_metadata(&stored)
                .expect("lstat")
                .is_symlink()
        );
        assert_eq!(std::fs::read(&stored).expect("read"), b"png");
        assert!(
            std::fs::symlink_metadata(&link).is_err(),
            "the link is gone"
        );
        assert!(target.exists(), "its target stays");
    }

    #[test]
    fn a_copy_of_a_symlink_stores_the_file() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let target = source(&session, "real.png", b"png");
        let link = session.root().join("incoming/link.png");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        add(session.layout(), &note, &[link], &opts()).expect("add");

        let stored = session.layout().asset_folder(FOLDER).join("link.png");
        assert!(
            !std::fs::symlink_metadata(&stored)
                .expect("lstat")
                .is_symlink()
        );
    }

    // ---- A source that is its own target ----

    /// A file already stored in the note's folder, and the folder.
    fn stored(session: &VaultSession, name: &str, content: &[u8]) -> (PathBuf, PathBuf) {
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join(name);
        std::fs::write(&path, content).expect("stored file");
        (folder, path)
    }

    /// Every way of asking for the same entry must be refused and leave the
    /// file and its bytes alone.
    fn assert_same_file_refused(session: &VaultSession, note: &Note, source: &Path, as_name: &str) {
        for (move_files, force) in [(false, false), (false, true), (true, false), (true, true)] {
            let options = AddOptions {
                move_files,
                force,
                rename: Some(as_name.to_string()),
            };
            let err = add(
                session.layout(),
                note,
                std::slice::from_ref(&source.to_path_buf()),
                &options,
            )
            .expect_err("a file is not added over itself");
            assert!(
                matches!(err, AssetError::SameFile { .. }),
                "move={move_files} force={force}: {err:?}"
            );
            assert_eq!(
                std::fs::read(source).expect("the file is still there"),
                b"keep",
                "move={move_files} force={force}"
            );
        }
    }

    #[test]
    fn a_file_already_in_the_folder_is_not_added_over_itself() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, path) = stored(&session, "a.pdf", b"keep");

        assert_same_file_refused(&session, &note, &path, "a.pdf");
    }

    #[test]
    fn the_same_file_reached_through_a_symlinked_directory_is_refused() {
        // The stand-in for a view's `_assets` link: a directory component that
        // is a symlink to the folder.
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (folder, path) = stored(&session, "a.pdf", b"keep");
        let via = session.root().join("via");
        std::os::unix::fs::symlink(&folder, &via).expect("symlink");

        assert_same_file_refused(&session, &note, &via.join("a.pdf"), "a.pdf");
        assert!(path.exists());
    }

    #[test]
    fn a_hard_link_of_the_target_is_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, path) = stored(&session, "a.pdf", b"keep");
        let hard = source(&session, "hard.pdf", b"x");
        std::fs::remove_file(&hard).expect("remove");
        std::fs::hard_link(&path, &hard).expect("hard link");

        assert_same_file_refused(&session, &note, &hard, "a.pdf");
    }

    #[test]
    fn a_symlink_standing_in_the_folder_is_not_added_over_itself() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (folder, _) = stored(&session, "other.pdf", b"x");
        let outside = source(&session, "outside.pdf", b"keep");
        let link = folder.join("link.pdf");
        std::os::unix::fs::symlink(&outside, &link).expect("symlink");

        assert_same_file_refused(&session, &note, &link, "link.pdf");
        assert_eq!(std::fs::read(&outside).expect("outside"), b"keep");
    }

    #[test]
    fn a_symlink_to_the_target_is_refused() {
        // The copy follows the link to the very file it is about to replace.
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, path) = stored(&session, "a.pdf", b"keep");
        let link = session.root().join("incoming-link.pdf");
        std::os::unix::fs::symlink(&path, &link).expect("symlink");

        assert_same_file_refused(&session, &note, &link, "a.pdf");
        assert_eq!(std::fs::read(&path).expect("target"), b"keep");
    }

    #[test]
    fn a_symlink_in_the_folder_is_replaced_by_the_file_it_points_at() {
        // Guard: the link is a different entry from its target, the copy goes
        // through a temp file, and only the link is replaced.
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (folder, _) = stored(&session, "other.pdf", b"x");
        let outside = source(&session, "a.pdf", b"keep");
        std::os::unix::fs::symlink(&outside, folder.join("a.pdf")).expect("symlink");
        let options = AddOptions {
            move_files: true,
            force: true,
            ..opts()
        };

        add(
            session.layout(),
            &note,
            std::slice::from_ref(&outside),
            &options,
        )
        .expect("a link to the source is replaced by the source");

        let target = folder.join("a.pdf");
        assert!(
            !std::fs::symlink_metadata(&target)
                .expect("lstat")
                .is_symlink()
        );
        assert_eq!(std::fs::read(&target).expect("read"), b"keep");
    }

    #[test]
    fn a_case_only_rename_is_the_same_file_where_the_filesystem_folds_case() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (folder, path) = stored(&session, "chart.pdf", b"keep");
        // The probe: does this filesystem find `CHART.PDF` for `chart.pdf`?
        if !folder.join("CHART.PDF").exists() {
            return;
        }

        assert_same_file_refused(&session, &note, &path, "Chart.pdf");
    }

    // ---- Names that differ only in case ----

    /// A source file `name` in the directory `incoming/<sub>/`.
    fn source_in(session: &VaultSession, sub: &str, name: &str, content: &[u8]) -> PathBuf {
        let dir = session.root().join("incoming").join(sub);
        std::fs::create_dir_all(&dir).expect("source directory");
        let path = dir.join(name);
        std::fs::write(&path, content).expect("source");
        path
    }

    fn clash_options(move_files: bool, force: bool) -> AddOptions {
        AddOptions {
            move_files,
            force,
            ..opts()
        }
    }

    /// Two sources in different directories, so the test runs on any
    /// filesystem, stored under names that fold to one.
    fn assert_two_names_clash(first: &str, second: &str) {
        for (move_files, force) in [(false, false), (false, true), (true, false), (true, true)] {
            let (_g, session) = vault_with_view();
            let note = note_with(&session, "body");
            let a = source_in(&session, "x", first, b"first");
            let b = source_in(&session, "y", second, b"second");

            let err = add(
                session.layout(),
                &note,
                &[a.clone(), b.clone()],
                &clash_options(move_files, force),
            )
            .expect_err("two names that fold to one are refused");

            assert!(
                matches!(err, AssetError::CaseClash { .. }),
                "{first} / {second} move={move_files} force={force}: {err:?}"
            );
            assert!(a.exists() && b.exists(), "nothing was moved");
            assert!(
                std::fs::read_dir(session.layout().asset_folder(FOLDER)).is_err(),
                "nothing was created"
            );
        }
    }

    #[test]
    fn two_sources_differing_only_in_case_are_refused() {
        assert_two_names_clash("shot.pdf", "Shot.pdf");
    }

    #[test]
    fn names_a_filesystem_folds_beyond_ascii_are_refused() {
        assert_two_names_clash("\u{17f}hot.pdf", "shot.pdf");
        assert_two_names_clash("Ma\u{df}.pdf", "MASS.pdf");
        assert_two_names_clash(
            "\u{39f}\u{394}\u{39f}\u{3a3}.pdf",
            "\u{3bf}\u{3b4}\u{3bf}\u{3c3}.pdf",
        );
    }

    #[test]
    fn a_source_differing_only_in_case_from_a_stored_file_is_refused_even_with_force() {
        for force in [false, true] {
            let (_g, session) = vault_with_view();
            let note = note_with(&session, "body");
            let (_, path) = stored(&session, "shot.pdf", b"keep");
            let file = source(&session, "Shot.pdf", b"new");

            let err = add(
                session.layout(),
                &note,
                std::slice::from_ref(&file),
                &clash_options(false, force),
            )
            .expect_err("a differently cased name is another spelling of the file");

            assert!(matches!(err, AssetError::CaseClash { .. }), "{err:?}");
            assert_eq!(std::fs::read(&path).expect("read"), b"keep");
        }
    }

    #[test]
    fn a_case_only_rename_next_to_the_stored_file_is_refused_on_every_filesystem() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, path) = stored(&session, "chart.pdf", b"keep");
        let file = source(&session, "x.pdf", b"new");
        let options = AddOptions {
            rename: Some("Chart.pdf".to_string()),
            force: true,
            ..opts()
        };

        let err = add(session.layout(), &note, &[file], &options).expect_err("case clash");

        assert!(matches!(err, AssetError::CaseClash { .. }), "{err:?}");
        assert_eq!(std::fs::read(&path).expect("read"), b"keep");
    }

    #[test]
    fn a_target_placed_earlier_in_the_run_is_recognised_by_its_entry() {
        let (_g, session) = vault_with_view();
        let (folder, path) = stored(&session, "a.pdf", b"first");
        let hard = folder.join("hard.pdf");
        std::fs::hard_link(&path, &hard).expect("hard link");
        let via = session.root().join("via");
        std::os::unix::fs::symlink(&folder, &via).expect("symlink");
        let mut placed = BTreeMap::new();
        placed.insert(identity_of(&path).expect("identity"), "a.pdf".to_string());

        assert_eq!(placed_earlier(&placed, &hard), Some("a.pdf".to_string()));
        assert_eq!(
            placed_earlier(&placed, &via.join("a.pdf")),
            Some("a.pdf".to_string())
        );
        assert_eq!(placed_earlier(&placed, &folder.join("absent.pdf")), None);
    }

    #[test]
    fn a_forced_placement_never_replaces_a_file_this_run_placed() {
        // Canonically equivalent spellings are one name on APFS, which the
        // up-front check does not fold; the second placement would replace the
        // first file there. Skipped where the filesystem keeps them apart.
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let nfc = "\u{e9}.pdf";
        let nfd = "e\u{301}.pdf";
        let probe = session.root().join(nfc);
        std::fs::write(&probe, b"x").expect("probe");
        if !session.root().join(nfd).exists() {
            return;
        }
        let a = source_in(&session, "x", nfc, b"first");
        let b = source_in(&session, "y", nfd, b"second");

        let err = add(session.layout(), &note, &[a, b], &clash_options(true, true))
            .expect_err("the second spelling is the first file");

        assert!(matches!(err, AssetError::CaseClash { .. }), "{err:?}");
        let folder = session.layout().asset_folder(FOLDER);
        let stored: Vec<_> = std::fs::read_dir(&folder).expect("folder").collect();
        assert_eq!(stored.len(), 1);
        assert_eq!(
            std::fs::read(stored[0].as_ref().expect("entry").path()).expect("read"),
            b"first"
        );
    }

    // ---- A symlinked assets directory or note folder ----

    /// A vault whose `all-notes/_assets` is a symlink to a directory outside
    /// it, and that directory.
    fn symlinked_assets_directory(session: &VaultSession) -> PathBuf {
        let outside = session.root().join("outside-assets");
        std::fs::create_dir_all(&outside).expect("outside");
        let assets = session.layout().assets_dir();
        let _ = std::fs::remove_dir_all(&assets);
        std::os::unix::fs::symlink(&outside, &assets).expect("symlink");
        outside
    }

    /// A vault whose folder for the note is a symlink to another note's
    /// folder, and that other folder.
    fn symlinked_note_folder(session: &VaultSession) -> PathBuf {
        let other = session
            .layout()
            .asset_folder("01BRZ3NDEKTSV4RRFFQ69G5FAV-other");
        std::fs::create_dir_all(&other).expect("other note's folder");
        std::os::unix::fs::symlink(&other, session.layout().asset_folder(FOLDER)).expect("symlink");
        other
    }

    fn entries(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .expect("directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn nothing_is_added_through_a_symlinked_assets_directory() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let outside = symlinked_assets_directory(&session);
        let file = source(&session, "a.pdf", b"x");

        let err = add(session.layout(), &note, &[file], &opts()).expect_err("refused");

        assert!(matches!(err, AssetError::Symlinked(_)), "{err:?}");
        assert!(entries(&outside).is_empty(), "{:?}", entries(&outside));
    }

    #[test]
    fn nothing_is_added_through_a_symlinked_note_folder() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let other = symlinked_note_folder(&session);
        let file = source(&session, "a.pdf", b"x");

        let err = add(session.layout(), &note, &[file], &opts()).expect_err("refused");

        assert!(matches!(err, AssetError::Symlinked(_)), "{err:?}");
        assert!(entries(&other).is_empty(), "{:?}", entries(&other));
    }

    #[test]
    fn a_dangling_note_folder_symlink_is_refused_too() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        std::fs::create_dir_all(session.layout().assets_dir()).expect("assets");
        std::os::unix::fs::symlink("/no-such-folder", session.layout().asset_folder(FOLDER))
            .expect("symlink");
        let file = source(&session, "a.pdf", b"x");

        let err = add(session.layout(), &note, &[file], &opts()).expect_err("refused");

        assert!(matches!(err, AssetError::Symlinked(_)), "{err:?}");
    }

    #[test]
    fn nothing_is_listed_through_a_symlinked_assets_directory() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let outside = symlinked_assets_directory(&session);
        std::fs::create_dir_all(outside.join(FOLDER)).expect("folder behind the link");
        std::fs::write(outside.join(FOLDER).join("secret.pdf"), b"x").expect("file");

        let err = list(session.layout(), &note).expect_err("refused");

        assert!(matches!(err, AssetError::Symlinked(_)), "{err:?}");
    }

    #[test]
    fn nothing_is_listed_through_a_symlinked_note_folder() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let other = symlinked_note_folder(&session);
        std::fs::write(other.join("theirs.pdf"), b"x").expect("file");

        let err = list(session.layout(), &note).expect_err("refused");

        assert!(matches!(err, AssetError::Symlinked(_)), "{err:?}");
    }

    // ---- A failure partway ----

    #[test]
    fn a_failure_on_a_later_file_reports_the_files_already_placed() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        // A directory of the second file's name: `--force` cannot replace it.
        std::fs::create_dir_all(folder.join("b.pdf")).expect("directory");
        let files = [
            source(&session, "a.pdf", b"a"),
            source(&session, "b.pdf", b"b"),
        ];
        let options = AddOptions {
            force: true,
            ..opts()
        };

        let report = super::add(session.layout(), &note, &files, &options);

        assert_eq!(report.added.len(), 1, "{report:?}");
        assert!(report.added[0].link.starts_with("[a.pdf]"));
        assert_eq!(std::fs::read(folder.join("a.pdf")).expect("read"), b"a");
        assert!(report.touched);
        let error = report.error.expect("the second file failed");
        assert!(
            matches!(&error, AssetError::Transfer { name, .. } if name == "b.pdf"),
            "{error:?}"
        );
    }

    #[test]
    fn a_move_whose_copy_fails_keeps_the_source_and_stores_nothing() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        // A directory of the file's name: `--force` cannot replace it, and a
        // forced move copies before it removes the source.
        std::fs::create_dir_all(folder.join("a.pdf")).expect("directory");
        let file = source(&session, "a.pdf", b"keep");
        let options = AddOptions {
            move_files: true,
            force: true,
            ..opts()
        };

        let report = super::add(
            session.layout(),
            &note,
            std::slice::from_ref(&file),
            &options,
        );

        assert!(report.added.is_empty(), "{report:?}");
        assert!(
            matches!(&report.error, Some(AssetError::Transfer { name, .. }) if name == "a.pdf"),
            "{report:?}"
        );
        assert_eq!(std::fs::read(&file).expect("the source stays"), b"keep");
    }

    #[test]
    fn a_failed_check_places_nothing_and_touches_nothing() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let files = [
            source(&session, "a.pdf", b"a"),
            source(&session, "no-such-file.pdf", b"x"),
        ];
        std::fs::remove_file(&files[1]).expect("remove");

        let report = super::add(session.layout(), &note, &files, &opts());

        assert!(report.added.is_empty());
        assert!(!report.touched);
        assert!(matches!(report.error, Some(AssetError::NotAFile(_))));
    }

    #[test]
    fn a_source_that_cannot_be_removed_after_its_copy_is_reported_as_stored() {
        use std::os::unix::fs::PermissionsExt;
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let locked = session.root().join("locked");
        std::fs::create_dir_all(&locked).expect("locked");
        let file = locked.join("a.pdf");
        std::fs::write(&file, b"keep").expect("source");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).expect("chmod");
        // A user that may remove it anyway (root) cannot show the failure.
        let probe = locked.join("probe");
        let can_write = std::fs::write(&probe, b"x").is_ok();
        let _ = std::fs::remove_file(&probe);
        if can_write {
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
                .expect("chmod back");
            return;
        }
        let options = AddOptions {
            move_files: true,
            force: true,
            ..opts()
        };

        let report = super::add(
            session.layout(),
            &note,
            std::slice::from_ref(&file),
            &options,
        );
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
            .expect("chmod back");

        assert_eq!(report.added.len(), 1, "{report:?}");
        assert_eq!(
            std::fs::read(&report.added[0].path).expect("stored"),
            b"keep"
        );
        assert!(file.exists(), "the source is still there");
        assert!(
            matches!(report.error, Some(AssetError::SourceKept { .. })),
            "{report:?}"
        );
    }

    // ---- Note files and another note's assets ----

    /// A second note, `01BRZ..-other`, with a file in its asset folder.
    fn other_note_with_asset(session: &VaultSession, name: &str) -> (String, PathBuf) {
        let folder_name = "01BRZ3NDEKTSV4RRFFQ69G5FAV-other";
        write_note(
            session,
            &format!("{folder_name}.md"),
            "---\ntitle: Other\n---\nbody\n",
        );
        let folder = session.layout().asset_folder(folder_name);
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join(name);
        std::fs::write(&path, b"theirs").expect("asset");
        (folder_name.to_string(), path)
    }

    fn assert_note_refused(session: &VaultSession, note: &Note, given: &Path, note_path: &Path) {
        for move_files in [false, true] {
            let options = AddOptions {
                move_files,
                ..opts()
            };
            let report = super::add(session.layout(), note, &[given.to_path_buf()], &options);
            assert!(
                matches!(report.error, Some(AssetError::NoteFile(_))),
                "move={move_files}: {report:?}"
            );
            assert!(report.added.is_empty());
            assert!(given.exists() && note_path.exists(), "move={move_files}");
        }
    }

    #[test]
    fn a_note_file_of_the_vault_is_not_added_as_an_asset() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");

        assert_note_refused(&session, &note, &note.path, &note.path);
    }

    #[test]
    fn an_encrypted_note_file_name_is_refused_too() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let age = session
            .layout()
            .all_notes()
            .join("01BRZ3NDEKTSV4RRFFQ69G5FAV-x.age");
        std::fs::write(&age, b"ciphertext").expect("age file");

        assert_note_refused(&session, &note, &age, &age);
    }

    #[test]
    fn another_spelling_of_the_notes_directory_is_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let spelled = session
            .layout()
            .all_notes()
            .join("..")
            .join("all-notes")
            .join(note.path.file_name().expect("name"));

        assert_note_refused(&session, &note, &spelled, &note.path);
    }

    #[test]
    fn a_view_leaf_that_is_a_symlink_to_a_note_is_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let leaf = session.root().join("leaf.md");
        std::os::unix::fs::symlink(&note.path, &leaf).expect("symlink");

        assert_note_refused(&session, &note, &leaf, &note.path);
    }

    #[test]
    fn a_note_reached_through_a_symlinked_directory_is_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let via = session.root().join("via");
        std::os::unix::fs::symlink(session.layout().all_notes(), &via).expect("symlink");
        let through = via.join(note.path.file_name().expect("name"));

        assert_note_refused(&session, &note, &through, &note.path);
    }

    fn move_options() -> AddOptions {
        AddOptions {
            move_files: true,
            ..opts()
        }
    }

    #[test]
    fn moving_a_file_out_of_another_notes_folder_warns_and_still_moves_it() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (folder_name, theirs) = other_note_with_asset(&session, "f.pdf");

        let report = super::add(
            session.layout(),
            &note,
            std::slice::from_ref(&theirs),
            &move_options(),
        );

        assert!(report.error.is_none(), "{report:?}");
        assert_eq!(report.added.len(), 1);
        assert!(!theirs.exists(), "the move still happens");
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains(&folder_name),
            "{:?}",
            report.warnings
        );
        assert!(
            report.warnings[0].contains("another note"),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn copying_a_file_of_another_notes_folder_does_not_warn() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, theirs) = other_note_with_asset(&session, "f.pdf");

        let report = super::add(
            session.layout(),
            &note,
            std::slice::from_ref(&theirs),
            &opts(),
        );

        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(theirs.exists());
    }

    #[test]
    fn a_folder_no_note_owns_gets_no_warning_when_a_file_is_moved_out() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let orphan = session
            .layout()
            .asset_folder("01CRZ3NDEKTSV4RRFFQ69G5FAV-gone");
        std::fs::create_dir_all(&orphan).expect("orphan folder");
        std::fs::write(orphan.join("f.pdf"), b"x").expect("file");

        let report = super::add(
            session.layout(),
            &note,
            &[orphan.join("f.pdf")],
            &move_options(),
        );

        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn a_symlink_in_another_notes_folder_is_the_entry_a_move_removes() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, theirs) = other_note_with_asset(&session, "f.pdf");
        let outside = source(&session, "outside.pdf", b"x");
        let link = theirs.parent().expect("folder").join("link.pdf");
        std::os::unix::fs::symlink(&outside, &link).expect("symlink");

        let report = super::add(
            session.layout(),
            &note,
            std::slice::from_ref(&link),
            &move_options(),
        );

        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(!link.exists(), "only the link is removed");
        assert!(outside.exists());
    }

    #[test]
    fn a_symlink_outside_pointing_into_another_notes_folder_removes_no_file_of_it() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let (_, theirs) = other_note_with_asset(&session, "f.pdf");
        let link = session.root().join("link.pdf");
        std::os::unix::fs::symlink(&theirs, &link).expect("symlink");

        let report = super::add(session.layout(), &note, &[link], &move_options());

        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(theirs.exists(), "the other note's file stays");
    }

    #[test]
    fn a_move_with_force_replaces_the_file_and_removes_the_source() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("a.png"), b"old").expect("existing");
        let file = source(&session, "a.png", b"new");
        let options = AddOptions {
            move_files: true,
            force: true,
            ..opts()
        };

        add(
            session.layout(),
            &note,
            std::slice::from_ref(&file),
            &options,
        )
        .expect("add");

        assert_eq!(std::fs::read(folder.join("a.png")).expect("read"), b"new");
        assert!(!file.exists());
    }

    #[test]
    fn files_land_in_a_folder_that_is_stale_after_a_retitle() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let old = session
            .layout()
            .asset_folder(&format!("{ULID}-an-older-title"));
        std::fs::create_dir_all(&old).expect("old folder");

        let added = add(
            session.layout(),
            &note,
            &[source(&session, "a.png", b"a")],
            &opts(),
        )
        .expect("add");

        assert!(added[0].path.starts_with(&old));
        assert!(added[0].link.contains("-an-older-title/a.png"));
        assert!(!session.layout().asset_folder(FOLDER).exists());
    }

    #[test]
    fn several_folders_of_the_note_are_refused() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        for name in ["a", "b"] {
            std::fs::create_dir_all(session.layout().asset_folder(&format!("{ULID}-{name}")))
                .expect("folder");
        }

        let err = add(
            session.layout(),
            &note,
            &[source(&session, "a.png", b"a")],
            &opts(),
        )
        .expect_err("two folders");

        assert!(err.to_string().contains("run `ntropy reconcile`"), "{err}");
    }

    // ---- list ----

    #[test]
    fn the_listing_marks_each_file_linked_or_not_and_names_a_missing_one() {
        let (_g, session) = vault_with_view();
        let note = note_with(
            &session,
            &format!("![c](_assets/{FOLDER}/chart.png) [m](_assets/{FOLDER}/gone.pdf)"),
        );
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(folder.join("sub")).expect("folder");
        std::fs::write(folder.join("chart.png"), b"12345").expect("file");
        std::fs::write(folder.join("old-draft.png"), b"12").expect("file");
        std::fs::write(folder.join("sub/n.txt"), b"1").expect("file");
        std::fs::write(folder.join(".DS_Store"), b"x").expect("hidden");

        let listing = list(session.layout(), &note).expect("list");

        let rows: Vec<(&str, Option<u64>, Linked)> = listing
            .files
            .iter()
            .map(|f| (f.name.as_str(), f.size, f.linked))
            .collect();
        assert_eq!(
            rows,
            [
                ("chart.png", Some(5), Linked::Yes),
                ("gone.pdf", None, Linked::Missing),
                ("old-draft.png", Some(2), Linked::No),
                ("sub/n.txt", Some(1), Linked::No),
            ]
        );
        assert_eq!(listing.folder, folder);
    }

    #[test]
    fn the_listing_shows_hidden_files_and_leaves_out_only_system_metadata() {
        // `delete` counts a `.env` and removes it, so the listing names it.
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        for name in [
            ".env",
            ".DS_Store",
            "._shot.png",
            "Thumbs.db",
            "desktop.ini",
            "a.pdf",
        ] {
            std::fs::write(folder.join(name), b"x").expect("file");
        }

        let listing = list(session.layout(), &note).expect("list");

        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, [".env", "a.pdf"]);
    }

    // ---- Raw HTML references ----

    fn listed(listing: &Listing) -> Vec<(&str, Option<u64>, Linked)> {
        listing
            .files
            .iter()
            .map(|f| (f.name.as_str(), f.size, f.linked))
            .collect()
    }

    #[test]
    fn a_file_the_note_uses_through_raw_html_is_listed_as_linked() {
        let (_g, session) = vault_with_view();
        let note = note_with(
            &session,
            &format!(
                "<img src=\"_assets/{FOLDER}/wide.png\" width=\"300\">\n\n\
                 <a href='_assets/{FOLDER}/my pic.png'>a picture</a>"
            ),
        );
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("wide.png"), b"1").expect("file");
        std::fs::write(folder.join("my pic.png"), b"12").expect("file");
        std::fs::write(folder.join("unused.png"), b"123").expect("file");

        let listing = list(session.layout(), &note).expect("list");

        assert_eq!(
            listed(&listing),
            [
                ("my pic.png", Some(2), Linked::Yes),
                ("unused.png", Some(3), Linked::No),
                ("wide.png", Some(1), Linked::Yes),
            ]
        );
    }

    #[test]
    fn raw_html_never_makes_a_file_missing() {
        // Only a Markdown link makes a file missing, as `reconcile` reports it.
        let (_g, session) = vault_with_view();
        let note = note_with(
            &session,
            &format!("<img src=\"_assets/{FOLDER}/gone.png\" width=\"300\">"),
        );

        let listing = list(session.layout(), &note).expect("list");

        assert!(listing.files.is_empty(), "{:?}", listing.files);
    }

    #[test]
    fn a_reference_in_code_does_not_make_a_file_linked() {
        let (_g, session) = vault_with_view();
        let note = note_with(
            &session,
            &format!(
                "`<img src=\"_assets/{FOLDER}/wide.png\">`\n\n```html\n<img src=\"_assets/{FOLDER}/wide.png\">\n```"
            ),
        );
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("wide.png"), b"1").expect("file");

        let listing = list(session.layout(), &note).expect("list");

        assert_eq!(listed(&listing), [("wide.png", Some(1), Linked::No)]);
    }

    #[test]
    fn a_link_into_the_file_through_a_stale_folder_name_counts_as_linked() {
        let (_g, session) = vault_with_view();
        let note = note_with(
            &session,
            &format!("![c](_assets/{ULID}-old-name/chart.png)"),
        );
        let folder = session.layout().asset_folder(&format!("{ULID}-old-name"));
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("chart.png"), b"x").expect("file");

        let listing = list(session.layout(), &note).expect("list");

        assert_eq!(listing.files[0].linked, Linked::Yes);
    }

    #[test]
    fn a_link_into_another_notes_folder_is_not_a_missing_file_of_this_one() {
        let (_g, session) = vault_with_view();
        let note = note_with(
            &session,
            "![c](_assets/01BRZ3NDEKTSV4RRFFQ69G5FAV-x/chart.png)",
        );

        let listing = list(session.layout(), &note).expect("list");

        assert!(listing.files.is_empty());
    }

    #[test]
    fn a_note_without_a_folder_lists_nothing_and_names_the_folder_it_would_have() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");

        let listing = list(session.layout(), &note).expect("list");

        assert!(listing.files.is_empty());
        assert_eq!(listing.folder, session.layout().asset_folder(FOLDER));
    }

    #[test]
    fn a_symlink_in_the_folder_is_listed_and_not_followed() {
        let (_g, session) = vault_with_view();
        let note = note_with(&session, "body");
        let folder = session.layout().asset_folder(FOLDER);
        std::fs::create_dir_all(&folder).expect("folder");
        std::os::unix::fs::symlink("/", folder.join("root")).expect("symlink");

        let listing = list(session.layout(), &note).expect("list");

        assert_eq!(listing.files.len(), 1);
        assert_eq!(listing.files[0].name, "root");
    }
}
