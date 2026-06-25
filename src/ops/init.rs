// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Vault initialization (ADR 0018).
//!
//! Scaffolds the directories and files a vault needs: `all-notes/`, `.ntropy/`,
//! the default template, and a per-vault config seeded with a `by-tag` view
//! (plus that view's directory). It is idempotent: each missing piece is
//! created and each existing piece is left untouched, so re-running `init`
//! always succeeds. It never touches the global config; the binary writes the
//! default-vault entry separately when `--set-default` is passed.

use std::path::{Path, PathBuf};

use crate::config::{PerVaultConfig, ViewConfig};
use crate::error::Result;
use crate::fsutil;
use crate::template::DEFAULT_TEMPLATE;
use crate::vault::Vault;
use crate::vault::layout;

/// The view seeded into a fresh vault.
const SEED_VIEW_NAME: &str = "by-tag";
const SEED_VIEW_FIELD: &str = "tags";

/// What `init` did, for human-facing reporting.
#[derive(Debug, Default)]
pub struct InitReport {
    /// The resolved vault root.
    pub root: PathBuf,
    /// Pieces newly created by this run (empty on a re-init of a complete
    /// vault).
    pub created: Vec<PathBuf>,
}

/// Initialize (or complete) a vault rooted at `path`.
pub fn init_vault(path: &Path) -> Result<InitReport> {
    let mut created = Vec::new();

    // The root and the well-known directories.
    ensure_dir(path, &mut created)?;
    let vault = Vault::new(path);
    let layout = vault.layout();

    ensure_dir(&layout.all_notes(), &mut created)?;
    ensure_dir(&layout.ntropy_dir(), &mut created)?;
    ensure_dir(&layout.templates_dir(), &mut created)?;

    // The default template, written only if absent so a customized one is kept.
    ensure_file(&layout.default_template(), DEFAULT_TEMPLATE, &mut created)?;

    // The per-vault config, seeded with the `by-tag` view on first creation.
    let config_path = layout.config_file();
    if !config_path.exists() {
        let mut config = PerVaultConfig::default();
        config.add(ViewConfig {
            name: SEED_VIEW_NAME.into(),
            field: SEED_VIEW_FIELD.into(),
        });
        fsutil::atomic_write(&config_path, config.to_toml()?.as_bytes())?;
        created.push(config_path);
        // The seeded view's directory, empty until notes carry tags.
        ensure_dir(&layout.view_dir(SEED_VIEW_NAME), &mut created)?;
    }

    Ok(InitReport {
        root: path.to_path_buf(),
        created,
    })
}

/// Create `dir` (and parents) if it does not yet exist, recording creation.
fn ensure_dir(dir: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        fsutil::create_dir_all(dir)?;
        created.push(dir.to_path_buf());
    }
    Ok(())
}

/// Write `contents` to `file` if it does not yet exist, recording creation.
fn ensure_file(file: &Path, contents: &str, created: &mut Vec<PathBuf>) -> Result<()> {
    if !file.exists() {
        fsutil::atomic_write(file, contents.as_bytes())?;
        created.push(file.to_path_buf());
    }
    Ok(())
}

/// Whether `path` is already an initialized vault.
pub fn is_initialized(path: &Path) -> bool {
    layout::is_vault(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffolds_a_fresh_vault() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("vault");
        let report = init_vault(&root).expect("init");

        let vault = Vault::new(&root);
        assert!(vault.layout().all_notes().is_dir());
        assert!(vault.layout().ntropy_dir().is_dir());
        assert!(vault.layout().templates_dir().is_dir());
        assert!(vault.layout().default_template().is_file());
        assert!(vault.layout().config_file().is_file());
        assert!(vault.layout().view_dir("by-tag").is_dir());
        assert!(!report.created.is_empty());
    }

    #[test]
    fn seeds_by_tag_view() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let config = PerVaultConfig::load(&Vault::new(dir.path()).layout().config_file())
            .expect("load config");
        assert_eq!(config.views.len(), 1);
        assert_eq!(config.views[0].name, "by-tag");
        assert_eq!(config.views[0].field, "tags");
    }

    #[test]
    fn default_template_is_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let template = std::fs::read_to_string(Vault::new(dir.path()).layout().default_template())
            .expect("read template");
        assert_eq!(template, DEFAULT_TEMPLATE);
    }

    #[test]
    fn is_idempotent() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("first init");
        // A second run creates nothing and still succeeds.
        let report = init_vault(dir.path()).expect("second init");
        assert!(report.created.is_empty());
    }

    #[test]
    fn preserves_customized_template_and_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let vault = Vault::new(dir.path());

        // Customize both, then re-init: customizations survive.
        std::fs::write(vault.layout().default_template(), "custom").expect("write template");
        let mut config = PerVaultConfig::default();
        config.add(ViewConfig {
            name: "by-status".into(),
            field: "status".into(),
        });
        std::fs::write(vault.layout().config_file(), config.to_toml().unwrap()).expect("write");

        init_vault(dir.path()).expect("re-init");
        assert_eq!(
            std::fs::read_to_string(vault.layout().default_template()).unwrap(),
            "custom"
        );
        let reloaded = PerVaultConfig::load(&vault.layout().config_file()).expect("reload");
        assert_eq!(reloaded.views[0].name, "by-status");
    }
}
