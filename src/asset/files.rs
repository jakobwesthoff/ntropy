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
use crate::note::Note;
use crate::vault::layout::{ALL_NOTES_DIR, ASSETS_DIR, Layout};

use super::{FolderChoice, Place, choose_folder, local_links, owner, place, read_folders};

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
pub fn folder_of(layout: &Layout, note: &Note) -> Result<PathBuf, AssetError> {
    let stem = note
        .path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let listed = read_folders(layout)?;
    match choose_folder(&listed, note.id, &stem) {
        FolderChoice::Existing(name) | FolderChoice::Create(name) => Ok(layout.asset_folder(&name)),
        FolderChoice::Several(names) => Err(AssetError::SeveralFolders(names)),
    }
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
/// a file, that each name can be linked, that no two files would be stored
/// under one name and, without `force`, that no file of that name is there.
/// A symlink as a source is followed for a copy. A move of one copies what it
/// points to and then removes the link itself, never its target, since moving
/// the link would put a symlink into the folder.
pub fn add(
    layout: &Layout,
    note: &Note,
    files: &[PathBuf],
    options: &AddOptions,
) -> Result<Vec<Added>, AssetError> {
    if options.rename.is_some() && files.len() != 1 {
        return Err(AssetError::RenameNeedsOneFile(files.len()));
    }
    let folder = folder_of(layout, note)?;
    let folder_name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut planned: Vec<(PathBuf, String, String)> = Vec::new();
    for source in files {
        if !std::fs::metadata(source).is_ok_and(|metadata| metadata.is_file()) {
            return Err(AssetError::NotAFile(source.clone()));
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
        if planned.iter().any(|(_, taken, _)| *taken == name) {
            return Err(AssetError::StoredTwice(name));
        }
        if !options.force && std::fs::symlink_metadata(folder.join(&name)).is_ok() {
            return Err(AssetError::Exists(name));
        }
        planned.push((source.clone(), name, link));
    }

    fsutil::create_dir_all(&folder)?;
    let mut added = Vec::new();
    for (source, name, link) in planned {
        let path = folder.join(&name);
        transfer(&source, &path, &folder, options)?;
        added.push(Added { path, link });
    }
    Ok(added)
}

/// Put `source` at `target`, inside `folder`.
fn transfer(
    source: &Path,
    target: &Path,
    folder: &Path,
    options: &AddOptions,
) -> Result<(), AssetError> {
    if !options.move_files {
        return Ok(fsutil::copy_file(source, target, options.force)?);
    }
    // A rename keeps one file on one device, and moves a symlink as the link,
    // so only a regular file on the folder's own device is renamed.
    let is_link = std::fs::symlink_metadata(source).is_ok_and(|m| m.file_type().is_symlink());
    let same_device = matches!(
        (std::fs::metadata(source), std::fs::metadata(folder)),
        (Ok(a), Ok(b)) if a.dev() == b.dev()
    );
    if !is_link && same_device && !options.force {
        return Ok(fsutil::rename(source, target)?);
    }
    fsutil::copy_file(source, target, options.force)?;
    Ok(fsutil::remove_file(source)?)
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

/// The files in the folder of `note`, hidden entries left out, and whether the
/// note links each. A file the note links in its own folder and the folder
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
            if name.starts_with('.') {
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

    let mut files: Vec<ListedFile> = found
        .iter()
        .map(|(name, size)| ListedFile {
            name: name.clone(),
            size: Some(*size),
            linked: if linked.contains(name) {
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
