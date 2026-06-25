// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Per-vault config: the materialized view definitions (ADR 0016).
//!
//! Stored at `<vault>/.ntropy/config.toml` as an array of `[[view]]` tables,
//! each pairing a view's output-directory `name` with the frontmatter `field`
//! it groups by. The `view list|add|remove` commands read and write this file
//! through the helpers here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ConfigError;

/// The parsed per-vault configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerVaultConfig {
    /// View definitions, serialized as `[[view]]` tables.
    #[serde(default, rename = "view")]
    pub views: Vec<ViewConfig>,
}

/// One view definition: an output directory name plus the field it groups by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewConfig {
    pub name: String,
    pub field: String,
}

impl PerVaultConfig {
    /// Load the config at `path`, treating a missing file as an empty config.
    ///
    /// A vault with no `config.toml` yet (or one that simply defines no views)
    /// is valid and yields no views rather than an error.
    pub fn load(path: &Path) -> Result<PerVaultConfig, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PerVaultConfig::default()),
            Err(source) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// Serialize the config to TOML text.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Find a view definition by name.
    pub fn find(&self, name: &str) -> Option<&ViewConfig> {
        self.views.iter().find(|v| v.name == name)
    }

    /// Add a view definition. Returns `false` if the name already exists.
    pub fn add(&mut self, view: ViewConfig) -> bool {
        if self.find(&view.name).is_some() {
            return false;
        }
        self.views.push(view);
        true
    }

    /// Remove a view definition by name. Returns `false` if it was absent.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.views.len();
        self.views.retain(|v| v.name != name);
        self.views.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str, field: &str) -> ViewConfig {
        ViewConfig {
            name: name.into(),
            field: field.into(),
        }
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cfg = PerVaultConfig::load(&dir.path().join("config.toml")).expect("load");
        assert_eq!(cfg, PerVaultConfig::default());
    }

    #[test]
    fn roundtrips_through_toml() {
        let mut cfg = PerVaultConfig::default();
        cfg.add(view("by-tag", "tags"));
        cfg.add(view("by-status", "status"));

        let text = cfg.to_toml().expect("serialize");
        let parsed: PerVaultConfig = toml::from_str(&text).expect("parse");
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn toml_uses_view_array_of_tables() {
        let mut cfg = PerVaultConfig::default();
        cfg.add(view("by-tag", "tags"));
        let text = cfg.to_toml().expect("serialize");
        insta::assert_snapshot!(text, @r#"
        [[view]]
        name = "by-tag"
        field = "tags"
        "#);
    }

    #[test]
    fn add_rejects_duplicate_name() {
        let mut cfg = PerVaultConfig::default();
        assert!(cfg.add(view("by-tag", "tags")));
        assert!(!cfg.add(view("by-tag", "topic")));
        assert_eq!(cfg.views.len(), 1);
    }

    #[test]
    fn remove_reports_presence() {
        let mut cfg = PerVaultConfig::default();
        cfg.add(view("by-tag", "tags"));
        assert!(cfg.remove("by-tag"));
        assert!(!cfg.remove("by-tag"));
        assert!(cfg.views.is_empty());
    }

    #[test]
    fn parse_invalid_toml_is_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not = = valid").expect("write");
        assert!(matches!(
            PerVaultConfig::load(&path),
            Err(ConfigError::Parse { .. })
        ));
    }
}
