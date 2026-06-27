// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Shared fixtures for the library's unit tests.
//!
//! Several modules exercise behavior against a real on-disk vault (view
//! materialization, reconciliation, deletion). They all need the same scaffold:
//! a temporary vault with `all-notes/`, a `.ntropy/` config, and one or more
//! configured views. These helpers build that scaffold so each test module does
//! not carry its own copy.

use std::path::PathBuf;

use tempfile::TempDir;

use crate::config::{PerVaultConfig, ViewConfig};
use crate::vault::Vault;

/// Build a temporary vault with `all-notes/` and the given `(name, field)`
/// views configured.
///
/// The returned [`TempDir`] guards the vault's lifetime: keep it bound for the
/// duration of the test, as dropping it removes the directory tree.
pub(crate) fn vault_with_views(views: &[(&str, &str)]) -> (TempDir, Vault) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");

    let mut config = PerVaultConfig::default();
    for (name, field) in views {
        config.add(ViewConfig {
            name: (*name).into(),
            field: (*field).into(),
        });
    }
    std::fs::write(
        root.join(".ntropy/config.toml"),
        config.to_toml().expect("toml"),
    )
    .expect("write config");

    let vault = Vault::new(root);
    (dir, vault)
}

/// Build a temporary vault with a single `by-tag` view grouping by `tags`.
pub(crate) fn vault_with_view() -> (TempDir, Vault) {
    vault_with_views(&[("by-tag", "tags")])
}

/// Write `content` to `all-notes/<name>` and return the note's path.
pub(crate) fn write_note(vault: &Vault, name: &str, content: &str) -> PathBuf {
    let path = vault.layout().all_notes().join(name);
    std::fs::write(&path, content).expect("write note");
    path
}
