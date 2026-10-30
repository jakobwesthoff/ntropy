// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Which vault files an html or site export may copy (ADR 0073).
//!
//! A note's link can name any path in the vault, and a linked directory is
//! mirrored with its whole tree; a theme's directory is copied the same way.
//! [`ExportScope`] answers, for every path an export is about to copy,
//! whether it may: an entry on [`NEVER_COPIED`] never, at any depth, and,
//! below a mirrored directory, no hidden entry either. A path named directly
//! is judged by [`ExportScope::verdict`], a directory by [`ExportScope::walk`].

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use crate::fsutil::{self, FsError};
use crate::vault::layout::{NTROPY_DIR, POINTER_FILE};

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
}

impl Refusal {
    /// The warning for a path named directly, `subject` saying what named it
    /// (`referenced path`) and `shown` the path as the reader knows it.
    pub fn message(&self, subject: &str, shown: &str) -> String {
        match self {
            Refusal::Listed(component) => format!(
                "{subject} `{shown}` is or lies inside `{component}`, which exports never copy; not copied"
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

    /// The warning about what this walk left out, if anything worth one:
    /// what `subject` (such as ``referenced directory `/x` ``) left out, the
    /// entries named by their path below `shown`.
    pub fn warning(&self, subject: &str, shown: &str) -> Option<String> {
        if self.ntropy.is_empty() {
            return None;
        }
        let paths: Vec<String> = self
            .ntropy
            .iter()
            .map(|relative| format!("`{}`", below(shown, relative)))
            .collect();
        Some(format!(
            "{subject} `{shown}`: left out {}",
            paths.join(", ")
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
    /// The base with every symlink resolved, or `None` when it cannot be
    /// resolved, so a resolved path can be judged below it.
    canonical_base: Option<PathBuf>,
    /// The export's own output, never walked: `(resolved parent, name)`,
    /// since the output may not exist yet when the walk runs.
    excluded: Vec<(PathBuf, OsString)>,
}

impl ExportScope {
    /// The scope below `base`, leaving out `excluded`, the export's own
    /// output.
    pub fn new(base: &Path, excluded: &[&Path]) -> Self {
        let excluded = excluded
            .iter()
            .filter_map(|path| {
                let parent = std::fs::canonicalize(path.parent()?).ok()?;
                Some((parent, path.file_name()?.to_os_string()))
            })
            .collect();
        ExportScope {
            base: base.to_path_buf(),
            canonical_base: std::fs::canonicalize(base).ok(),
            excluded,
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
        if let Some(component) = self.resolved_listed(path) {
            return Err(Refusal::Listed(component));
        }
        Ok(())
    }

    /// The listed component of where `path` resolves to, below the base.
    fn resolved_listed(&self, path: &Path) -> Option<String> {
        let canonical_base = self.canonical_base.as_ref()?;
        let resolved = std::fs::canonicalize(path).ok()?;
        listed_component(resolved.strip_prefix(canonical_base).ok()?)
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
        let mut walk = Walk::default();
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
            if file_type.is_symlink()
                && let Some(component) = self.resolved_listed(&path)
            {
                walk.left_out(component, relative);
                continue;
            }
            if file_type.is_dir() {
                self.walk_into(&path, &relative, walk)?;
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
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[]);
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
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[]);
        assert_eq!(
            scope.verdict(&scope.vault_path("/assets/missing.png")),
            Ok(())
        );
    }

    #[test]
    fn href_path_keeps_a_refused_or_climbing_link_as_written() {
        let scope = ExportScope::new(Path::new("/no-such-vault"), &[]);
        assert_eq!(
            scope.href_path("/all-notes", "../assets/x.png").as_deref(),
            Some("/assets/x.png")
        );
        assert_eq!(scope.href_path("/all-notes", "/.ssh/id_ed25519"), None);
        assert_eq!(scope.href_path("/all-notes", "../../x.png"), None);
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
        let scope = ExportScope::new(vault.path(), &[&artifact, &files_dir]);
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
                ..Walk::default()
            }
        );
    }
}
