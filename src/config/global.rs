// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Global config: the default vault (ADR 0016).
//!
//! Located in the OS-native config directory via the `directories` crate
//! (`~/.config/ntropy/config.toml` on Linux, `~/Library/Application
//! Support/ntropy/config.toml` on macOS). The only v1 field is `default_vault`,
//! used when no `--vault`, `$NTROPY_VAULT` or cwd walk-up resolves a vault.
//!
//! The path-resolving helpers and the IO helpers are kept separate: `*_at`
//! functions take an explicit path so they are testable against a temp file,
//! while the no-suffix functions resolve the real OS location.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use super::ConfigError;

/// The config file name within the application config directory.
const CONFIG_FILE: &str = "config.toml";

/// The parsed global configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// The fallback vault path used when nothing else resolves one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_vault: Option<PathBuf>,
}

/// The OS-native path to the global config file, if a config dir is known.
pub fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "ntropy").map(|dirs| dirs.config_dir().join(CONFIG_FILE))
}

/// Load the global config from `path`, treating a missing file as default.
pub fn load_at(path: &Path) -> Result<GlobalConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(GlobalConfig::default()),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Write the global config to `path`, creating parent directories as needed.
pub fn write_at(path: &Path, config: &GlobalConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let text = toml::to_string_pretty(config)?;
    std::fs::write(path, text).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Load the global config from its OS-native location.
///
/// Returns the default config when no config directory is known or the file is
/// absent, so callers always get a usable value.
pub fn load() -> Result<GlobalConfig, ConfigError> {
    match config_path() {
        Some(path) => load_at(&path),
        None => Ok(GlobalConfig::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_is_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cfg = load_at(&dir.path().join("config.toml")).expect("load");
        assert_eq!(cfg, GlobalConfig::default());
        assert!(cfg.default_vault.is_none());
    }

    #[test]
    fn write_then_load_roundtrips() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A nested path exercises parent-directory creation.
        let path = dir.path().join("ntropy").join("config.toml");
        let cfg = GlobalConfig {
            default_vault: Some(PathBuf::from("/Users/jakob/notes")),
        };
        write_at(&path, &cfg).expect("write");
        assert_eq!(load_at(&path).expect("load"), cfg);
    }

    #[test]
    fn default_serializes_without_vault_key() {
        let text = toml::to_string_pretty(&GlobalConfig::default()).expect("serialize");
        assert!(!text.contains("default_vault"), "got: {text:?}");
    }

    #[test]
    fn config_path_is_under_ntropy_dir() {
        // The exact root is OS-dependent, but the file always sits under an
        // `ntropy` directory and is named `config.toml`.
        if let Some(path) = config_path() {
            assert!(path.ends_with("ntropy/config.toml"));
        }
    }
}
