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

use crate::error::Result;
use crate::note::Note;
use crate::vault::{InvalidViewName, Vault, ViewName};

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
}

impl SkippedView {
    fn invalid(name: &str, reason: InvalidViewName) -> Self {
        SkippedView {
            name: name.to_string(),
            reason: SkipReason::InvalidName(reason),
            suggestion: ViewName::suggest(name),
        }
    }
}

impl fmt::Display for SkippedView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The configured name is untrusted and, being invalid, may carry
        // control characters; it is shown escaped.
        write!(
            f,
            "skipped view `{}`: {}",
            self.name.escape_debug(),
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
pub fn sync_all(vault: &Vault, views: &[ViewDef], notes: &[Note]) -> Result<()> {
    for view in views {
        sync_view(vault, view, notes)?;
    }
    Ok(())
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

    #[test]
    fn view_def_parse_rejects_an_invalid_name() {
        assert_eq!(
            ViewDef::parse("../x", "tags"),
            Err(InvalidViewName::DotSegment("..".to_string()))
        );
    }
}
