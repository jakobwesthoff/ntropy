// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The delete use case (ADR 0018).
//!
//! Removes a note's canonical file and then refreshes the views, so the deleted
//! note's links disappear from every view tree. Selector resolution and the
//! confirmation prompt live in the binary; this is the headless effect.

use std::path::Path;

use crate::error::Result;
use crate::fsutil;
use crate::reconcile;
use crate::scan::ScanWarning;
use crate::vault::Vault;

/// Delete the note file at `path` and rebuild the views.
///
/// Returns the scan warnings produced while rebuilding (so a caller can honor
/// `--strict`).
pub fn delete_note(vault: &Vault, path: &Path) -> Result<Vec<ScanWarning>> {
    fsutil::remove_file(path)?;
    reconcile::refresh_views(vault)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PerVaultConfig, ViewConfig};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn vault_with_view() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        let mut config = PerVaultConfig::default();
        config.add(ViewConfig {
            name: "by-tag".into(),
            field: "tags".into(),
        });
        std::fs::write(
            root.join(".ntropy/config.toml"),
            config.to_toml().expect("toml"),
        )
        .expect("write config");
        let vault = Vault::new(root);
        (dir, vault)
    }

    #[test]
    fn removes_file_and_prunes_links() {
        let (_g, vault) = vault_with_view();
        let path = vault.layout().all_notes().join(format!("{ULID}-note.md"));
        std::fs::write(&path, "---\ntitle: Note\ntags: [work]\n---\n").expect("write");

        // Build the view first so there is a link to prune.
        reconcile::refresh_views(&vault).expect("refresh");
        assert!(vault.root().join("by-tag/work").is_dir());

        delete_note(&vault, &path).expect("delete");
        assert!(!path.exists());
        // The note's group is gone after the rebuild.
        assert!(!vault.root().join("by-tag/work").exists());
    }
}
