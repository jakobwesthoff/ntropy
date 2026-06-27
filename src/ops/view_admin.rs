// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The view administration use cases: `view list|add|remove` (ADR 0018).
//!
//! These read and write the per-vault config and keep the derived state in
//! step: adding a view materializes it from the current notes, and both add and
//! remove sync the root `.gitignore` to the configured views (ADR 0032).
//! Removing a view never deletes its directory — ntropy leaves the now-stale
//! tree in place for the user to remove, pruning only the managed ignore entry.
//! Editing a view is intentionally absent in v1 (it is remove + add).

use crate::config::{PerVaultConfig, ViewConfig};
use crate::error::Result;
use crate::fsutil;
use crate::gitignore;
use crate::scan;
use crate::vault::Vault;
use crate::vault::layout;
use crate::view::{self, ViewDef};

/// Why a view administration command was rejected.
#[derive(Debug, thiserror::Error)]
pub enum ViewAdminError {
    #[error("`{0}` is a reserved name and cannot be a view")]
    ReservedName(String),
    #[error("a view named `{0}` already exists")]
    Duplicate(String),
    #[error("no view named `{0}`")]
    NotFound(String),
}

/// List the configured views.
pub fn list_views(vault: &Vault) -> Result<Vec<ViewConfig>> {
    let config = PerVaultConfig::load(&vault.layout().config_file())?;
    Ok(config.views)
}

/// Add a view named `name` grouping by `field`, then materialize it.
pub fn add_view(vault: &Vault, name: &str, field: &str) -> Result<()> {
    if layout::is_reserved_name(name) {
        return Err(ViewAdminError::ReservedName(name.to_string()).into());
    }

    let config_path = vault.layout().config_file();
    let mut config = PerVaultConfig::load(&config_path)?;
    let added = config.add(ViewConfig {
        name: name.to_string(),
        field: field.to_string(),
    });
    if !added {
        return Err(ViewAdminError::Duplicate(name.to_string()).into());
    }
    fsutil::atomic_write(&config_path, config.to_toml()?.as_bytes())?;

    // Materialize the new view immediately so its directory reflects the
    // current notes.
    let scan = scan::scan_notes_dir(&vault.layout().all_notes())?;
    view::build_view(vault, &ViewDef::new(name, field), &scan.notes)?;

    // Keep `.gitignore` in step with the now-larger view set.
    sync_gitignore(vault, &config)?;
    Ok(())
}

/// Remove the view named `name` from config and prune its `.gitignore` entry.
///
/// The view's directory is intentionally left on disk; ntropy never deletes a
/// directory. The returned report names the pruned entry so the caller can tell
/// the user the directory remains.
pub fn remove_view(vault: &Vault, name: &str) -> Result<gitignore::SyncReport> {
    let config_path = vault.layout().config_file();
    let mut config = PerVaultConfig::load(&config_path)?;
    if !config.remove(name) {
        return Err(ViewAdminError::NotFound(name.to_string()).into());
    }
    fsutil::atomic_write(&config_path, config.to_toml()?.as_bytes())?;
    sync_gitignore(vault, &config)
}

/// Sync the root `.gitignore` to the views in `config`.
fn sync_gitignore(vault: &Vault, config: &PerVaultConfig) -> Result<gitignore::SyncReport> {
    let names: Vec<&str> = config.views.iter().map(|v| v.name.as_str()).collect();
    gitignore::sync(vault, &names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn temp_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let vault = Vault::new(dir.path());
        (dir, vault)
    }

    #[test]
    fn add_then_list() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-status", "status").expect("add");
        let views = list_views(&v).expect("list");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].name, "by-status");
        assert!(v.layout().view_dir("by-status").is_dir());
    }

    #[test]
    fn add_writes_gitignore_entry() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-status", "status").expect("add");
        let gitignore = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        assert!(gitignore.contains("/by-status/"), "got: {gitignore}");
    }

    #[test]
    fn add_materializes_existing_notes() {
        let (_g, v) = temp_vault();
        std::fs::write(
            v.layout().all_notes().join(format!("{ULID}-n.md")),
            "---\ntitle: N\nstatus: Done\n---\n",
        )
        .expect("write note");
        add_view(&v, "by-status", "status").expect("add");
        assert!(v.root().join("by-status/done").is_dir());
    }

    #[test]
    fn add_rejects_reserved_name() {
        let (_g, v) = temp_vault();
        let err = add_view(&v, "all-notes", "tags").expect_err("reserved");
        assert!(matches!(
            err,
            Error::ViewAdmin(ViewAdminError::ReservedName(_))
        ));
    }

    #[test]
    fn add_rejects_duplicate() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-tag", "tags").expect("add");
        let err = add_view(&v, "by-tag", "tags").expect_err("dup");
        assert!(matches!(
            err,
            Error::ViewAdmin(ViewAdminError::Duplicate(_))
        ));
    }

    #[test]
    fn remove_keeps_directory_and_prunes_config_and_gitignore() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-tag", "tags").expect("add");
        assert!(v.layout().view_dir("by-tag").is_dir());

        let report = remove_view(&v, "by-tag").expect("remove");
        assert!(list_views(&v).expect("list").is_empty());
        // ntropy never deletes the directory; the stale tree is left in place.
        assert!(v.layout().view_dir("by-tag").exists());
        // Only the managed ignore entry is pruned.
        assert_eq!(report.removed, ["/by-tag/"]);
        let gitignore = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        assert!(
            !gitignore.contains("/by-tag/"),
            "entry not pruned: {gitignore}"
        );
    }

    #[test]
    fn remove_unknown_is_error() {
        let (_g, v) = temp_vault();
        let err = remove_view(&v, "ghost").expect_err("missing");
        assert!(matches!(err, Error::ViewAdmin(ViewAdminError::NotFound(_))));
    }
}
