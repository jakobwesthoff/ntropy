// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Materialized views: projecting notes into symlink trees (ADRs 0008, 0009).
//!
//! A [`ViewDef`] pairs an output-directory name with the frontmatter field it
//! groups by. The view layer is deliberately independent of the config layer:
//! it operates on `ViewDef` values that a caller derives from config, so the
//! dependency only ever points down to `note`/`vault`/`fsutil`.

pub mod leaf;
pub mod materialize;

use crate::error::Result;
use crate::note::Note;
use crate::vault::Vault;

pub use materialize::build_view;

/// A view definition: an output directory name plus the field it groups by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewDef {
    pub name: String,
    pub field: String,
}

impl ViewDef {
    pub fn new(name: impl Into<String>, field: impl Into<String>) -> Self {
        ViewDef {
            name: name.into(),
            field: field.into(),
        }
    }
}

/// Rebuild every given view from the current note set.
///
/// This is the full-rebuild path ntropy uses to keep views fresh after a
/// mutation and during `reconcile`: each view directory is regenerated from
/// scratch, which is always correct and prunes stale links (ADR 0008; the v1
/// deviation from literal incremental updates is tracked as follow-up work).
pub fn rebuild_all(vault: &Vault, views: &[ViewDef], notes: &[Note]) -> Result<()> {
    for view in views {
        build_view(vault, view, notes)?;
    }
    Ok(())
}
