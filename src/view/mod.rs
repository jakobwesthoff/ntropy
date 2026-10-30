// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Materialized views: projecting notes into symlink trees (ADRs 0008, 0009).
//!
//! A [`ViewDef`] pairs an output-directory name with the frontmatter field it
//! groups by. The view layer is deliberately independent of the config layer:
//! it operates on `ViewDef` values that a caller derives from config, so the
//! dependency only ever points down to `note`/`vault`/`fsutil`. Callers hand
//! the raw configured pairs to [`parse_defs`], which keeps the views whose
//! names are valid and reports the rest as [`SkippedView`]s (ADR 0064).

pub mod leaf;
pub mod materialize;

use std::fmt;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::note::Note;
use crate::vault::{InvalidViewName, Layout, Vault, ViewName, escape_controls};

pub use materialize::{group_values, sync_view};

/// A view definition: an output directory name plus the field it groups by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewDef {
    pub name: ViewName,
    pub field: String,
}

impl ViewDef {
    pub fn new(name: ViewName, field: impl Into<String>) -> Self {
        ViewDef {
            name,
            field: field.into(),
        }
    }

    /// A definition from a raw name, if the name is valid.
    pub fn parse(
        name: &str,
        field: impl Into<String>,
    ) -> std::result::Result<Self, InvalidViewName> {
        Ok(ViewDef::new(ViewName::parse(name)?, field))
    }
}

/// A configured view that ntropy left alone, and why.
#[derive(Debug)]
pub struct SkippedView {
    /// The name as configured.
    pub name: String,
    pub reason: SkipReason,
    /// A valid name close to the configured one, offered to the user.
    pub suggestion: Option<ViewName>,
}

/// Why a configured view was skipped.
#[derive(Debug, thiserror::Error)]
pub enum SkipReason {
    #[error(transparent)]
    InvalidName(InvalidViewName),
    #[error(
        "`{}` is a symlink, and syncing through it could reach outside the vault",
        .0.display()
    )]
    Symlinked(PathBuf),
    #[error("`{}` could not be inspected: {source}", .path.display())]
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl SkippedView {
    fn invalid(name: &str, reason: InvalidViewName) -> Self {
        SkippedView {
            name: name.to_string(),
            reason: SkipReason::InvalidName(reason),
            suggestion: ViewName::suggest(name),
        }
    }

    /// A valid view whose directory could not be used safely.
    pub fn unsafe_dir(name: &ViewName, error: ViewDirError) -> Self {
        let reason = match error {
            ViewDirError::Symlinked(path) => SkipReason::Symlinked(path),
            ViewDirError::Inspect { path, source } => SkipReason::Unreadable { path, source },
        };
        SkippedView {
            name: name.to_string(),
            reason,
            suggestion: None,
        }
    }
}

/// Why a view's directory cannot be synced or removed safely.
#[derive(Debug, thiserror::Error)]
pub enum ViewDirError {
    #[error(
        "`{}` is a symlink, and syncing through it could reach outside the vault",
        .0.display()
    )]
    Symlinked(PathBuf),
    #[error("`{}` could not be inspected", .path.display())]
    Inspect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// The directory of the view `name`, once it is known to stay inside the
/// vault.
///
/// A valid name keeps the path inside the vault only as long as nothing on the
/// way is a symlink: the sync lists and empties the directory by path, and a
/// symlink at the view's root, or at any segment above it, would carry every
/// one of those calls to wherever it points. Each segment below the vault root
/// is therefore inspected without following it. The vault root itself is not:
/// resolution canonicalizes it.
///
/// The walk stops at the first segment that does not exist, since everything
/// below it is created as real directories, and at the first one that exists
/// but is not a directory, which creating the view's directory then reports.
///
/// The check and the sync's later calls are separate path lookups, so a
/// symlink swapped in between them is not caught (ADR 0064).
pub fn contained_dir(
    layout: &Layout,
    name: &ViewName,
) -> std::result::Result<PathBuf, ViewDirError> {
    let mut path = layout.root().to_path_buf();
    for segment in name.segments() {
        path.push(segment);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(ViewDirError::Symlinked(path));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(source) => return Err(ViewDirError::Inspect { path, source }),
        }
    }
    Ok(layout.view_dir(name))
}

impl fmt::Display for SkippedView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The configured name is untrusted and, being invalid, may carry
        // control characters; it is shown escaped.
        write!(
            f,
            "skipped view `{}`: {}",
            escape_controls(&self.name),
            self.reason
        )?;
        if let Some(suggestion) = &self.suggestion {
            write!(f, " (try `{suggestion}`)")?;
        }
        Ok(())
    }
}

/// Split configured `(name, field)` pairs into valid definitions and skipped
/// views, both in configuration order.
///
/// A view with an invalid name is skipped rather than failing the whole
/// configuration, so one bad entry never stops the others from syncing, and a
/// skipped view is never turned into a path.
pub fn parse_defs<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> (Vec<ViewDef>, Vec<SkippedView>) {
    let mut views = Vec::new();
    let mut skipped = Vec::new();
    for (name, field) in entries {
        match ViewDef::parse(name, field) {
            Ok(view) => views.push(view),
            Err(reason) => skipped.push(SkippedView::invalid(name, reason)),
        }
    }
    (views, skipped)
}

/// Sync every given view to the current note set.
///
/// The path ntropy uses to keep views fresh after a mutation and during
/// `reconcile`: each view is diffed against its on-disk tree and only the changed
/// links are touched, which is always correct and prunes stale links (ADR 0008).
///
/// A view whose directory sits behind a symlink is skipped and returned, and
/// the others are synced; any other failure stops the sync.
pub fn sync_all(vault: &Vault, views: &[ViewDef], notes: &[Note]) -> Result<Vec<SkippedView>> {
    let mut skipped = Vec::new();
    for view in views {
        match sync_view(vault, view, notes) {
            Ok(()) => {}
            Err(Error::View(error @ ViewDirError::Symlinked(_))) => {
                skipped.push(SkippedView::unsafe_dir(&view.name, error));
            }
            Err(other) => return Err(other),
        }
    }
    Ok(skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defs_splits_valid_and_invalid_in_order() {
        let (views, skipped) = parse_defs([
            ("by-tag", "tags"),
            ("../decoy", "tags"),
            ("by-status", "status"),
            ("By_Tag", "tags"),
        ]);
        let names: Vec<&str> = views.iter().map(|view| view.name.as_str()).collect();
        assert_eq!(names, ["by-tag", "by-status"]);
        assert_eq!(views[1].field, "status");
        let skipped_names: Vec<&str> = skipped.iter().map(|view| view.name.as_str()).collect();
        assert_eq!(skipped_names, ["../decoy", "By_Tag"]);
    }

    #[test]
    fn a_skipped_view_explains_itself() {
        let (_, skipped) = parse_defs([("../decoy", "tags")]);
        assert_eq!(
            skipped[0].to_string(),
            "skipped view `../decoy`: the segment `..` starts with a dot (try `decoy`)"
        );
    }

    #[test]
    fn a_skipped_view_without_a_suggestion_says_only_why() {
        let (_, skipped) = parse_defs([("all-notes", "tags")]);
        assert_eq!(
            skipped[0].to_string(),
            "skipped view `all-notes`: `all-notes` is reserved by ntropy"
        );
    }

    #[test]
    fn a_skipped_view_name_is_shown_escaped() {
        let (_, skipped) = parse_defs([("by\ntag", "tags")]);
        let message = skipped[0].to_string();
        assert!(message.starts_with("skipped view `by\\ntag`"), "{message}");
        assert!(!message.contains('\n'), "{message}");
    }

    // -- contained_dir -------------------------------------------------------

    fn name(raw: &str) -> ViewName {
        ViewName::parse(raw).expect("valid view name")
    }

    /// A vault at `<base>/vault`, plus a directory `<base>/outside` that a
    /// planted symlink can point at.
    fn vault_and_outside() -> (tempfile::TempDir, Layout, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("vault");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).expect("vault");
        std::fs::create_dir_all(outside.join("work")).expect("outside");
        (dir, Layout::new(root), outside)
    }

    #[test]
    fn contained_dir_accepts_real_and_missing_directories() {
        let (_g, layout, _outside) = vault_and_outside();
        std::fs::create_dir_all(layout.root().join("by-tag")).expect("view dir");
        std::fs::create_dir_all(layout.root().join("area")).expect("area");

        for view in ["by-tag", "by-status", "area/work"] {
            let dir = contained_dir(&layout, &name(view)).expect("contained");
            assert_eq!(dir, layout.view_dir(&name(view)));
        }
    }

    #[test]
    fn contained_dir_refuses_a_symlinked_view_root() {
        let (_g, layout, outside) = vault_and_outside();
        let link = layout.root().join("by-tag");
        std::os::unix::fs::symlink(&outside, &link).expect("plant symlink");

        match contained_dir(&layout, &name("by-tag")) {
            Err(ViewDirError::Symlinked(path)) => assert_eq!(path, link),
            other => panic!("expected a symlink refusal, got {other:?}"),
        }
    }

    #[test]
    fn contained_dir_refuses_a_symlinked_intermediate_segment() {
        let (_g, layout, outside) = vault_and_outside();
        let link = layout.root().join("area");
        std::os::unix::fs::symlink(&outside, &link).expect("plant symlink");

        match contained_dir(&layout, &name("area/work")) {
            Err(ViewDirError::Symlinked(path)) => assert_eq!(path, link),
            other => panic!("expected a symlink refusal, got {other:?}"),
        }
    }

    #[test]
    fn contained_dir_leaves_a_regular_file_to_directory_creation() {
        // A file where a directory belongs is no escape; creating the view's
        // directory reports it as it always has.
        let (_g, layout, _outside) = vault_and_outside();
        std::fs::write(layout.root().join("by-tag"), "a file").expect("file");
        std::fs::write(layout.root().join("area"), "a file").expect("file");

        assert!(contained_dir(&layout, &name("by-tag")).is_ok());
        assert!(contained_dir(&layout, &name("area/work")).is_ok());
    }

    #[test]
    fn contained_dir_reports_a_path_it_cannot_inspect() {
        let (_g, layout, _outside) = vault_and_outside();
        let view = crate::test_support::uninspectable_view(layout.root());

        match contained_dir(&layout, &view) {
            Err(ViewDirError::Inspect { path, .. }) => {
                assert!(path.starts_with(layout.root()));
            }
            other => panic!("expected an inspection failure, got {other:?}"),
        }
    }

    #[test]
    fn sync_all_skips_only_a_symlinked_view_and_stops_on_other_failures() {
        let (_g, layout, outside) = vault_and_outside();
        let vault = Vault::new(layout.root());
        std::os::unix::fs::symlink(&outside, layout.root().join("linked")).expect("plant");
        let views = [
            ViewDef::new(name("linked"), "tags"),
            ViewDef::new(name("by-tag"), "tags"),
        ];
        let skipped = sync_all(&vault, &views, &[]).expect("sync");
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].name, "linked");
        assert!(layout.root().join("by-tag").is_dir());

        let long = crate::test_support::uninspectable_view(layout.root());
        let err = sync_all(&vault, &[ViewDef::new(long, "tags")], &[]).expect_err("fails");
        assert!(
            matches!(err, Error::View(ViewDirError::Inspect { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn unsafe_directories_explain_themselves() {
        let symlinked = ViewDirError::Symlinked("/v/by-tag".into());
        assert_eq!(
            symlinked.to_string(),
            "`/v/by-tag` is a symlink, and syncing through it could reach outside the vault"
        );
        let skipped = SkippedView::unsafe_dir(&name("by-tag"), symlinked);
        assert_eq!(
            skipped.to_string(),
            "skipped view `by-tag`: `/v/by-tag` is a symlink, and syncing through it could \
             reach outside the vault"
        );

        let inspect = ViewDirError::Inspect {
            path: "/v/long".into(),
            source: std::io::Error::other("name too long"),
        };
        assert_eq!(inspect.to_string(), "`/v/long` could not be inspected");
        let skipped = SkippedView::unsafe_dir(&name("long"), inspect);
        assert_eq!(
            skipped.to_string(),
            "skipped view `long`: `/v/long` could not be inspected: name too long"
        );
    }

    #[test]
    fn view_def_parse_rejects_an_invalid_name() {
        assert_eq!(
            ViewDef::parse("../x", "tags"),
            Err(InvalidViewName::DotSegment("..".to_string()))
        );
    }
}
