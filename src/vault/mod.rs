// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The vault: a resolved storage context (ADRs 0007, 0016).
//!
//! A [`Vault`] is just a validated root directory plus the [`Layout`] that
//! computes its well-known paths. It deliberately knows nothing about config
//! contents or scanning; higher layers combine it with those. Keeping it that
//! thin is what lets it sit at the bottom of the dependency graph alongside the
//! filesystem and kernel primitives.
//!
//! Alongside it, [`seed`] carries the content those well-known files start out
//! with, embedded from a file tree at compile time.

pub mod layout;
pub mod resolve;
pub mod seed;

use std::path::{Path, PathBuf};

pub use layout::Layout;
pub use resolve::{ResolveError, ResolveOptions, ResolveSource};

/// A resolved vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vault {
    layout: Layout,
}

impl Vault {
    /// Wrap an already-resolved vault root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Vault {
            layout: Layout::new(root),
        }
    }

    /// Resolve a vault from the given options (ADRs 0016, 0026).
    pub fn resolve(opts: &ResolveOptions) -> Result<Vault, ResolveError> {
        Ok(Vault::new(resolve::resolve(opts)?))
    }

    /// The path helpers for this vault.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The vault root directory.
    pub fn root(&self) -> &Path {
        self.layout.root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_root_into_layout() {
        let vault = Vault::new("/vault");
        assert_eq!(vault.root(), Path::new("/vault"));
        assert_eq!(
            vault.layout().all_notes(),
            PathBuf::from("/vault/all-notes")
        );
    }
}
