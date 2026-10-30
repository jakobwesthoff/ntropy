// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Which vault files an html or site export may copy (ADR 0073).
//!
//! A note's link can name any path in the vault, and a linked directory is
//! mirrored with its whole tree; a theme's directory is copied the same way.
//! [`ExportScope`] answers, for every path an export is about to copy,
//! whether it may: only what resolves inside the vault (for a theme file:
//! inside the theme or the vault), an entry on [`NEVER_COPIED`] never, at
//! any depth, a note outside the export never, and, below a mirrored
//! directory, no hidden entry either. A
//! path named directly is judged by [`ExportScope::verdict`], a directory by
//! [`ExportScope::walk`].

use std::collections::{BTreeSet, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use crate::fsutil::{self, FileId, FsError};
use crate::vault::layout::{self, ALL_NOTES_DIR, NTROPY_DIR, POINTER_FILE};

// =========================================================
// The names never copied
// =========================================================

/// Names an export never copies (ADR 0073), as a directory or a file, at any
/// depth below the linked directory or the theme, on the path as written and
/// on the path it resolves to. Matching ignores ASCII case, since a
/// case-insensitive filesystem such as APFS or NTFS opens `.NTROPY` as
/// `.ntropy`, except for `CVS`, which matches its exact case, because `CVs`
/// is a plausible folder of résumés.
///
/// An entry belongs here when it is one of:
/// - a note tool's configuration or pointer: `.ntropy`, `.ntropy-vault`,
///   `.obsidian`;
/// - version control, whose history holds deleted and unexported notes:
///   `.bzr`, `.fslckout`, `.git`, `.hg`, `.jj`, `.pijul`, `.svn`, `CVS`,
///   `_darcs`;
/// - a sync tool's state: `.dropbox.cache`, `.stfolder`, `.stversions`;
/// - trash: `$RECYCLE.BIN`, and `.Trash` as a prefix, which covers
///   `.Trashes`, `.Trash-1000` and Obsidian's `.trash`;
/// - credentials: `.aws`, `.env`, `.gnupg`, `.netrc`, `.ssh`;
/// - file-manager metadata that reveals other files: the Finder's
///   `.DS_Store` names a folder's files, Windows Explorer's `Thumbs.db`
///   holds thumbnails of its images.
///
/// Kept in byte order of [`Refused::text`]; this is the one list, and the
/// user docs group the same names by these categories. A name that fits no
/// category needs a category here first.
pub const NEVER_COPIED: [Refused; 24] = [
    Refused::Name("$RECYCLE.BIN"),
    Refused::Name(".DS_Store"),
    Refused::Prefix(".Trash"),
    Refused::Name(".aws"),
    Refused::Name(".bzr"),
    Refused::Name(".dropbox.cache"),
    Refused::Name(".env"),
    Refused::Name(".fslckout"),
    Refused::Name(".git"),
    Refused::Name(".gnupg"),
    Refused::Name(".hg"),
    Refused::Name(".jj"),
    Refused::Name(".netrc"),
    Refused::Name(NTROPY_DIR),
    Refused::Name(POINTER_FILE),
    Refused::Name(".obsidian"),
    Refused::Name(".pijul"),
    Refused::Name(".ssh"),
    Refused::Name(".stfolder"),
    Refused::Name(".stversions"),
    Refused::Name(".svn"),
    Refused::ExactName("CVS"),
    Refused::Name("Thumbs.db"),
    Refused::Name("_darcs"),
];

/// One entry of [`NEVER_COPIED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A name, ASCII case ignored.
    Name(&'static str),
    /// A name in exactly this case.
    ExactName(&'static str),
    /// Every name that starts with this, ASCII case ignored.
    Prefix(&'static str),
}

impl Refused {
    /// The name or prefix as listed.
    pub fn text(&self) -> &'static str {
        match self {
            Refused::Name(text) | Refused::ExactName(text) | Refused::Prefix(text) => text,
        }
    }

    /// Whether `name`, one path component, is this entry. The comparison is
    /// on bytes, so a name that is not UTF-8 matches only where its bytes
    /// spell the entry.
    pub fn matches(&self, name: &OsStr) -> bool {
        let name = name.as_encoded_bytes();
        match self {
            Refused::Name(text) => name.eq_ignore_ascii_case(text.as_bytes()),
            Refused::ExactName(text) => name == text.as_bytes(),
            Refused::Prefix(text) => name
                .get(..text.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(text.as_bytes())),
        }
    }
}

/// Whether `name` is on [`NEVER_COPIED`].
fn never_copied(name: &OsStr) -> bool {
    NEVER_COPIED.iter().any(|entry| entry.matches(name))
}

/// The first component of `path` on [`NEVER_COPIED`], as found.
fn listed_component(path: &Path) -> Option<String> {
    path.components().find_map(|component| match component {
        Component::Normal(name) if never_copied(name) => Some(name.to_string_lossy().into_owned()),
        _ => None,
    })
}

/// Whether a matched component is the vault's configuration directory, whose
/// appearance in an export is always worth a warning.
fn is_ntropy(component: &str) -> bool {
    component.eq_ignore_ascii_case(NTROPY_DIR)
}

// =========================================================
// Verdicts
// =========================================================

/// Why a path is not copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The path is, or lies inside, an entry of [`NEVER_COPIED`]; the
    /// component as written or as found on disk.
    Listed(String),
    /// The path resolves outside what the export may read from; how that
    /// reads in a warning, such as `the vault`.
    Outside(&'static str),
    /// The path exists but does not resolve: a dangling symlink, a loop, a
    /// directory on the way that cannot be read.
    Unresolvable,
    /// The path resolves to a note outside the export. `scanned` tells
    /// whether the scan reads the file as a note, so a warning does not call
    /// a file a note that ntropy itself never reads as one.
    Unexported { scanned: bool },
}

impl Refusal {
    /// The warning for a path named directly, `by` saying what named it
    /// (`referenced`) and `shown` the path as the reader knows it.
    pub fn message(&self, by: &str, shown: &str) -> String {
        match self {
            Refusal::Listed(component) => format!(
                "{by} path `{shown}` is or lies inside `{component}`, which exports never copy; not copied"
            ),
            Refusal::Outside(reach) => {
                format!("{by} path `{shown}` resolves outside {reach}; not copied")
            }
            Refusal::Unresolvable => {
                format!("{by} path `{shown}` cannot be resolved; not copied")
            }
            Refusal::Unexported { scanned: true } => {
                format!("{by} note `{shown}` is not among the exported notes; not copied")
            }
            Refusal::Unexported { scanned: false } => format!(
                "{by} file `{shown}` sits directly in {ALL_NOTES_DIR}/ and is not among the exported notes; not copied"
            ),
        }
    }
}

/// What a mirrored directory holds that may be copied, and what it left out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Walk {
    /// Every file to copy as `(path relative to the directory with `/`
    /// separators, source)`, sorted.
    pub files: Vec<(String, PathBuf)>,
    /// `.ntropy` entries left out, relative to the directory.
    pub ntropy: Vec<String>,
    /// How many other [`NEVER_COPIED`] entries were left out.
    pub listed: usize,
    /// The distinct names of those entries, as found.
    pub listed_names: BTreeSet<String>,
    /// How many hidden entries were left out.
    pub hidden: usize,
    /// How many entries resolve outside what the export may read from.
    pub outside: usize,
    /// How many entries exist but do not resolve.
    pub unresolvable: usize,
    /// How many symlinks to a directory were not followed.
    pub symlinked_dirs: usize,
    /// How many notes outside the export were left out.
    pub unexported: usize,
    /// How many files directly in `all-notes/` that the scan would not read
    /// as notes were left out as notes outside the export.
    pub unscanned: usize,
    /// What the export may read from, for the warning's wording.
    pub reach: &'static str,
}

/// How many distinct names a note line spells out before it counts the rest.
const SHOWN_NAMES: usize = 5;

impl Walk {
    /// Record a left-out entry at `relative` that matched `component`.
    fn left_out(&mut self, component: String, relative: String) {
        if is_ntropy(&component) {
            self.ntropy.push(relative);
        } else {
            self.listed += 1;
            self.listed_names.insert(component);
        }
    }

    /// Record a note outside the export.
    fn unexported_note(&mut self, scanned: bool) {
        if scanned {
            self.unexported += 1;
        } else {
            self.unscanned += 1;
        }
    }

    /// The warning about what this walk left out, if anything worth one:
    /// what `subject` (such as ``referenced directory `/x` ``) left out,
    /// counted by kind in a fixed order, and the `.ntropy` entries named by
    /// their path below `shown`.
    pub fn warning(&self, subject: &str, shown: &str) -> Option<String> {
        let mut parts = Vec::new();
        if self.unexported > 0 {
            parts.push(counted(
                self.unexported,
                "note outside the export",
                "notes outside the export",
            ));
        }
        if self.unscanned > 0 {
            parts.push(counted(
                self.unscanned,
                &format!("file directly in {ALL_NOTES_DIR}/ outside the export"),
                &format!("files directly in {ALL_NOTES_DIR}/ outside the export"),
            ));
        }
        if self.outside > 0 {
            let reach = self.reach;
            parts.push(counted(
                self.outside,
                &format!("entry that resolves outside {reach}"),
                &format!("entries that resolve outside {reach}"),
            ));
        }
        if self.unresolvable > 0 {
            parts.push(counted(
                self.unresolvable,
                "entry that cannot be resolved",
                "entries that cannot be resolved",
            ));
        }
        if self.symlinked_dirs > 0 {
            parts.push(counted(
                self.symlinked_dirs,
                "symlinked directory",
                "symlinked directories",
            ));
        }
        parts.extend(
            self.ntropy
                .iter()
                .map(|relative| format!("`{}`", below(shown, relative))),
        );
        if parts.is_empty() {
            return None;
        }
        Some(format!(
            "{subject} `{shown}`: left out {}",
            parts.join(", ")
        ))
    }

    /// The note about the routine metadata this walk left out, if any.
    pub fn note(&self, subject: &str, shown: &str) -> Option<String> {
        let mut parts = Vec::new();
        if self.hidden > 0 {
            parts.push(counted(self.hidden, "hidden entry", "hidden entries"));
        }
        if self.listed > 0 {
            let names: Vec<String> = self
                .listed_names
                .iter()
                .take(SHOWN_NAMES)
                .map(|name| format!("`{name}`"))
                .collect();
            let rest = self.listed_names.len().saturating_sub(SHOWN_NAMES);
            let mut shown_names = names.join(", ");
            if rest > 0 {
                shown_names.push_str(&format!(" and {rest} more"));
            }
            parts.push(format!(
                "{} ({shown_names})",
                counted(self.listed, "never-copied entry", "never-copied entries")
            ));
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!(
            "{subject} `{shown}`: left out {}",
            parts.join(" and ")
        ))
    }
}

/// `count` with its unit in the singular or the plural.
fn counted(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

/// The extension the scan reads the notes of the vault at `root` by.
fn scanned_extension(root: &Path) -> &'static str {
    if layout::is_encrypted(root) {
        "age"
    } else {
        "md"
    }
}

/// The resolved form of each of `dirs` that can be resolved.
fn resolved(dirs: &[&Path]) -> Vec<PathBuf> {
    dirs.iter()
        .filter_map(|dir| std::fs::canonicalize(dir).ok())
        .collect()
}

/// `relative` below `shown`, a `/`-separated path, without a doubled slash.
fn below(shown: &str, relative: &str) -> String {
    format!("{}/{relative}", shown.trim_end_matches('/'))
}

// =========================================================
// The scope of one export
// =========================================================

/// What one export may copy from below a base directory: the vault root for
/// the files notes reference, a theme's directory for the theme's files.
#[derive(Debug, Clone)]
pub struct ExportScope {
    /// The base as given; the paths an export builds start with it.
    base: PathBuf,
    /// The resolved directories a copied file must resolve into, in order:
    /// the vault root, or a theme's directory and then the vault root. A
    /// directory that cannot be resolved is left out, so with none left
    /// nothing that exists is copied.
    boundaries: Vec<PathBuf>,
    /// How the boundaries read in a warning.
    reach: &'static str,
    /// The export's own output, never walked: `(resolved parent, name)`,
    /// since the output may not exist yet when the walk runs.
    excluded: Vec<(PathBuf, OsString)>,
    /// The vault's notes directory, `None` when it cannot be read.
    notes_dir: Option<FileId>,
    /// The extension the scan reads notes by in this vault: `age` in an
    /// encrypted vault, `md` in a plaintext one.
    scanned_extension: &'static str,
    /// The note files the export holds; every other note stays out.
    exported: HashSet<FileId>,
}

impl ExportScope {
    /// The scope of the files notes reference, below the vault root:
    /// `exported` are the files of the notes the export holds, `excluded`
    /// the export's own output.
    pub fn new(vault_root: &Path, exported: &[&Path], excluded: &[&Path]) -> Self {
        let excluded = excluded
            .iter()
            .filter_map(|path| {
                let parent = std::fs::canonicalize(path.parent()?).ok()?;
                Some((parent, path.file_name()?.to_os_string()))
            })
            .collect();
        ExportScope {
            base: vault_root.to_path_buf(),
            boundaries: resolved(&[vault_root]),
            reach: "the vault",
            excluded,
            notes_dir: fsutil::file_id(&vault_root.join(ALL_NOTES_DIR)),
            scanned_extension: scanned_extension(vault_root),
            exported: exported
                .iter()
                .filter_map(|path| fsutil::file_id(path))
                .collect(),
        }
    }

    /// The scope of a theme's files: below its directory, resolving into it
    /// or into the vault. The second boundary lets a theme reach a vault
    /// asset; the first keeps a theme kept outside the vault usable. A
    /// theme carries no note, so every note it reaches stays out.
    pub fn for_theme(theme_dir: &Path, vault_root: &Path) -> Self {
        ExportScope {
            base: theme_dir.to_path_buf(),
            boundaries: resolved(&[theme_dir, vault_root]),
            reach: "the theme and the vault",
            excluded: Vec::new(),
            notes_dir: fsutil::file_id(&vault_root.join(ALL_NOTES_DIR)),
            scanned_extension: scanned_extension(vault_root),
            exported: HashSet::new(),
        }
    }

    /// The file a root-absolute path from a note names, such as `/assets/x`.
    pub fn vault_path(&self, root_absolute: &str) -> PathBuf {
        self.base.join(root_absolute.trim_start_matches('/'))
    }

    /// Whether `path`, below the base, may be copied when named directly.
    ///
    /// The name rule runs on the path as written and on the path it resolves
    /// to, so neither a case variant nor a symlink gets an entry of
    /// [`NEVER_COPIED`] past it. A path that does not exist passes, so the
    /// caller reports it as missing.
    pub fn verdict(&self, path: &Path) -> Result<(), Refusal> {
        if let Some(component) = path
            .strip_prefix(&self.base)
            .ok()
            .and_then(listed_component)
        {
            return Err(Refusal::Listed(component));
        }
        self.resolve(path).map(|_| ())
    }

    /// Where `path` resolves, if it exists and may be read from there.
    ///
    /// Every unclear case is refused: a path that exists but does not
    /// resolve, and one that resolves outside every boundary (compared by
    /// path components, so `/vault2` does not count as inside `/vault`). A
    /// path that resolves inside is checked by name below the boundary that
    /// holds it, which catches a symlink to a listed entry, and by the note
    /// rule, which catches a view's leaf to a note outside the export.
    pub fn resolve(&self, path: &Path) -> Result<Option<PathBuf>, Refusal> {
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            _ => {}
        }
        let resolved = std::fs::canonicalize(path).map_err(|_| Refusal::Unresolvable)?;
        let below = self
            .boundaries
            .iter()
            .find_map(|boundary| resolved.strip_prefix(boundary).ok())
            .ok_or(Refusal::Outside(self.reach))?;
        if let Some(component) = listed_component(below) {
            return Err(Refusal::Listed(component));
        }
        let in_notes_dir = resolved
            .parent()
            .is_some_and(|parent| self.is_notes_dir(parent));
        match self.unexported(&resolved, in_notes_dir) {
            Some(refusal) => Err(refusal),
            None => Ok(Some(resolved)),
        }
    }

    /// Whether `dir` is the vault's notes directory, by identity, so a
    /// symlink or a case variant leading there counts too.
    fn is_notes_dir(&self, dir: &Path) -> bool {
        self.notes_dir
            .is_some_and(|notes_dir| fsutil::file_id(dir) == Some(notes_dir))
    }

    /// The refusal for `file` when it is a note outside the export. A note,
    /// for an export, is a regular file directly in `all-notes/` ending in
    /// `.md` or `.age` in any letter case, whatever the vault's storage
    /// form: wider than what the scan reads, since a stray plaintext note in
    /// an encrypted vault, an `.age` file left by an interrupted conversion
    /// and a case variant hold note text all the same.
    fn unexported(&self, file: &Path, in_notes_dir: bool) -> Option<Refusal> {
        if !in_notes_dir {
            return None;
        }
        let extension = file.extension()?;
        if !(extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("age")) {
            return None;
        }
        if !std::fs::metadata(file).is_ok_and(|metadata| metadata.is_file()) {
            return None;
        }
        // A note whose identity cannot be read is not known to be part of
        // the export, so it stays out.
        let exported = fsutil::file_id(file).is_some_and(|id| self.exported.contains(&id));
        (!exported).then_some(Refusal::Unexported {
            scanned: extension == self.scanned_extension,
        })
    }

    /// The root-absolute vault path a page links for `dest`, written in a
    /// note in `note_dir`, or `None` when the link keeps its text: the path
    /// climbs out of the vault (ADR 0072) or is refused.
    pub fn href_path(&self, note_dir: &str, dest: &str) -> Option<String> {
        let path = crate::render::markdown::resolve_root_relative(note_dir, dest)?;
        self.verdict(&self.vault_path(&path)).ok().map(|()| path)
    }

    /// Every file below `dir` that may be copied, and what was left out.
    /// `dir` itself is judged by [`ExportScope::verdict`] first.
    pub fn walk(&self, dir: &Path) -> Result<Walk, FsError> {
        let mut walk = Walk {
            reach: self.reach,
            ..Walk::default()
        };
        self.walk_into(dir, "", &mut walk)?;
        walk.files.sort();
        walk.ntropy.sort();
        Ok(walk)
    }

    fn walk_into(&self, dir: &Path, prefix: &str, walk: &mut Walk) -> Result<(), FsError> {
        // The export's own output can sit in the walked tree, such as an
        // artifact rendered into the vault; it is compared by resolved
        // parent and name, so read the directory's resolved path once.
        let here = if self.excluded.is_empty() {
            None
        } else {
            std::fs::canonicalize(dir).ok()
        };
        // Only a file directly in `all-notes/` can be a note; asked once per
        // directory rather than once per file.
        let in_notes_dir = self.is_notes_dir(dir);
        for (path, file_type) in fsutil::read_dir_entries(dir)? {
            let name = path
                .file_name()
                .expect("a directory entry has a name")
                .to_os_string();
            let relative = if prefix.is_empty() {
                name.to_string_lossy().into_owned()
            } else {
                format!("{prefix}/{}", name.to_string_lossy())
            };
            if let Some(here) = &here
                && self
                    .excluded
                    .iter()
                    .any(|(parent, excluded)| parent == here && *excluded == name)
            {
                continue;
            }
            if never_copied(&name) {
                walk.left_out(name.to_string_lossy().into_owned(), relative);
                continue;
            }
            if name.as_encoded_bytes().starts_with(b".") {
                walk.hidden += 1;
                continue;
            }
            if file_type.is_symlink() {
                // A symlink is copied from where it points, so it is judged
                // there; one to a directory is not followed, which keeps a
                // loop or a second path into the same tree out of the walk.
                match self.resolve(&path) {
                    Err(Refusal::Listed(component)) => walk.left_out(component, relative),
                    Err(Refusal::Outside(_)) => walk.outside += 1,
                    Err(Refusal::Unexported { scanned }) => walk.unexported_note(scanned),
                    // A symlink entry exists, so `Ok(None)`, "not found",
                    // cannot come back; counted with the unresolvable
                    // ones all the same rather than copied.
                    Err(Refusal::Unresolvable) | Ok(None) => walk.unresolvable += 1,
                    Ok(Some(resolved)) if resolved.is_dir() => walk.symlinked_dirs += 1,
                    Ok(Some(_)) => walk.files.push((relative, path)),
                }
            } else if file_type.is_dir() {
                self.walk_into(&path, &relative, walk)?;
            } else if let Some(Refusal::Unexported { scanned }) =
                self.unexported(&path, in_notes_dir)
            {
                walk.unexported_note(scanned);
            } else {
                walk.files.push((relative, path));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_copied_is_sorted_and_no_entry_matches_another() {
        for pair in NEVER_COPIED.windows(2) {
            assert!(
                pair[0].text().as_bytes() < pair[1].text().as_bytes(),
                "`{}` sorts before `{}`",
                pair[1].text(),
                pair[0].text()
            );
        }
        for a in &NEVER_COPIED {
            for b in &NEVER_COPIED {
                if a != b {
                    assert!(
                        !b.matches(OsStr::new(a.text())),
                        "`{}` already covers `{}`",
                        b.text(),
                        a.text()
                    );
                }
            }
        }
    }

    #[test]
    fn every_never_copied_entry_is_refused_as_directory_and_file_at_any_depth() {
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[], &[]);
        for entry in &NEVER_COPIED {
            let name = entry.text();
            for written in [
                format!("/{name}/inside"),
                format!("/a/b/{name}"),
                format!("/a/{name}/inside"),
            ] {
                assert_eq!(
                    scope.verdict(&scope.vault_path(&written)),
                    Err(Refusal::Listed(name.to_string())),
                    "{written}"
                );
            }
        }
    }

    #[test]
    fn a_name_ignores_ascii_case() {
        assert!(Refused::Name(".git").matches(OsStr::new(".GIT")));
        assert!(!Refused::Name(".git").matches(OsStr::new(".gitkeep")));
    }

    #[test]
    fn an_exact_name_keeps_its_case() {
        assert!(Refused::ExactName("CVS").matches(OsStr::new("CVS")));
        assert!(!Refused::ExactName("CVS").matches(OsStr::new("CVs")));
    }

    #[test]
    fn a_prefix_covers_every_name_starting_with_it() {
        let entry = Refused::Prefix(".no-such-prefix");
        assert!(entry.matches(OsStr::new(".no-such-prefix")));
        assert!(entry.matches(OsStr::new(".NO-SUCH-PREFIX-1000")));
        assert!(!entry.matches(OsStr::new("x.no-such-prefix")));
        assert!(!entry.matches(OsStr::new(".no-such")));
    }

    #[test]
    fn the_trash_prefix_covers_every_trash_folder() {
        for name in [".Trash", ".Trashes", ".Trash-1000", ".trash"] {
            assert!(never_copied(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn other_hidden_names_are_not_on_the_list() {
        for name in [".well-known", ".gitkeep", ".editorconfig", ".environment"] {
            assert!(!never_copied(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn a_path_that_does_not_exist_passes_unless_its_name_is_listed() {
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[], &[]);
        assert_eq!(
            scope.verdict(&scope.vault_path("/assets/missing.png")),
            Ok(())
        );
    }

    #[test]
    fn href_path_keeps_a_refused_or_climbing_link_as_written() {
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[], &[]);
        assert_eq!(
            scope.href_path("/all-notes", "../assets/x.png").as_deref(),
            Some("/assets/x.png")
        );
        assert_eq!(scope.href_path("/all-notes", "/.ssh/id_ed25519"), None);
        assert_eq!(scope.href_path("/all-notes", "../../x.png"), None);
    }

    #[test]
    fn a_sibling_whose_name_extends_the_vault_s_is_outside() {
        let parent = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(parent.path().join("vault")).expect("vault");
        std::fs::create_dir_all(parent.path().join("vault2")).expect("sibling");
        std::fs::write(parent.path().join("vault2/x.png"), "png").expect("file");
        std::os::unix::fs::symlink("../vault2/x.png", parent.path().join("vault/x.png"))
            .expect("symlink");
        let scope = ExportScope::new(&parent.path().join("vault"), &[], &[]);
        assert_eq!(
            scope.verdict(&scope.vault_path("/x.png")),
            Err(Refusal::Outside("the vault"))
        );
    }

    #[test]
    fn a_vault_root_that_cannot_be_resolved_refuses_what_exists() {
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[], &[]);
        assert_eq!(
            scope.verdict(elsewhere.path()),
            Err(Refusal::Outside("the vault"))
        );
    }

    #[test]
    fn a_symlink_loop_cannot_be_resolved() {
        let vault = tempfile::tempdir().expect("temp dir");
        std::os::unix::fs::symlink("b", vault.path().join("a")).expect("symlink a");
        std::os::unix::fs::symlink("a", vault.path().join("b")).expect("symlink b");
        let scope = ExportScope::new(vault.path(), &[], &[]);
        assert_eq!(
            scope.verdict(&scope.vault_path("/a")),
            Err(Refusal::Unresolvable)
        );
        let walk = scope.walk(vault.path()).expect("the walk reads the vault");
        assert_eq!(walk.unresolvable, 2);
        assert!(walk.files.is_empty(), "{:?}", walk.files);
    }

    #[test]
    fn a_note_is_called_one_by_the_storage_form_the_scan_reads() {
        let vault = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(vault.path().join("all-notes")).expect("all-notes");
        std::fs::write(vault.path().join("all-notes/sealed.age"), "age").expect("age");
        std::fs::write(vault.path().join("all-notes/plain.md"), "md").expect("md");
        let plaintext = ExportScope::new(vault.path(), &[], &[]);
        assert_eq!(
            plaintext.verdict(&plaintext.vault_path("/all-notes/sealed.age")),
            Err(Refusal::Unexported { scanned: false })
        );
        assert_eq!(
            plaintext.verdict(&plaintext.vault_path("/all-notes/plain.md")),
            Err(Refusal::Unexported { scanned: true })
        );

        std::fs::create_dir_all(vault.path().join(".ntropy")).expect(".ntropy");
        std::fs::write(vault.path().join(".ntropy/identity.pub"), "age1").expect("recipient");
        let encrypted = ExportScope::new(vault.path(), &[], &[]);
        assert_eq!(
            encrypted.verdict(&encrypted.vault_path("/all-notes/sealed.age")),
            Err(Refusal::Unexported { scanned: true })
        );
        assert_eq!(
            encrypted.verdict(&encrypted.vault_path("/all-notes/plain.md")),
            Err(Refusal::Unexported { scanned: false })
        );
    }

    #[test]
    fn a_theme_reaches_no_note() {
        let vault = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(vault.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(vault.path().join("theme")).expect("theme");
        std::fs::write(vault.path().join("all-notes/note.md"), "md").expect("note");
        std::os::unix::fs::symlink("../all-notes/note.md", vault.path().join("theme/note.md"))
            .expect("symlink");
        let scope = ExportScope::for_theme(&vault.path().join("theme"), vault.path());
        let walk = scope
            .walk(&vault.path().join("theme"))
            .expect("the walk reads the theme");
        assert!(walk.files.is_empty(), "{:?}", walk.files);
        assert_eq!(walk.unexported, 1);
    }

    #[test]
    fn a_walk_reports_by_category() {
        let mut walk = Walk::default();
        assert_eq!(walk.warning("referenced directory", "/x"), None);
        assert_eq!(walk.note("referenced directory", "/x"), None);

        walk.left_out(".ntropy".to_string(), ".ntropy".to_string());
        walk.left_out(".NTROPY".to_string(), "sub/.NTROPY".to_string());
        assert_eq!(
            walk.warning("referenced directory", "/").as_deref(),
            Some("referenced directory `/`: left out `/.ntropy`, `/sub/.NTROPY`")
        );

        walk.hidden = 1;
        assert_eq!(
            walk.note("referenced directory", "/x").as_deref(),
            Some("referenced directory `/x`: left out 1 hidden entry")
        );
        walk.left_out(".git".to_string(), "a/.git".to_string());
        assert_eq!(
            walk.note("referenced directory", "/x").as_deref(),
            Some(
                "referenced directory `/x`: left out 1 hidden entry and 1 never-copied entry (`.git`)"
            )
        );
    }

    #[test]
    fn a_note_names_at_most_five_entries() {
        let mut walk = Walk::default();
        for name in [".a1", ".a2", ".a3", ".a4", ".a5", ".a6", ".a7"] {
            walk.left_out(name.to_string(), name.to_string());
            walk.left_out(name.to_string(), format!("sub/{name}"));
        }
        assert_eq!(
            walk.note("theme", "corporate").as_deref(),
            Some(
                "theme `corporate`: left out 14 never-copied entries (`.a1`, `.a2`, `.a3`, `.a4`, `.a5` and 2 more)"
            )
        );
    }

    #[test]
    fn the_export_s_own_output_is_not_walked() {
        let vault = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(vault.path().join("x_files")).expect("files dir");
        std::fs::write(vault.path().join("x_files/style.css"), "css").expect("style");
        std::fs::write(vault.path().join("pic.png"), "png").expect("asset");
        let files_dir = vault.path().join("x_files");
        let artifact = vault.path().join("x.html");
        let scope = ExportScope::new(vault.path(), &[], &[&artifact, &files_dir]);
        let walk = scope.walk(vault.path()).expect("the walk reads the vault");
        let files: Vec<&str> = walk
            .files
            .iter()
            .map(|(relative, _)| relative.as_str())
            .collect();
        assert_eq!(files, ["pic.png"]);
        assert_eq!(
            walk,
            Walk {
                files: walk.files.clone(),
                reach: "the vault",
                ..Walk::default()
            }
        );
    }
}
