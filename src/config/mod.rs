// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Configuration in two tiers (ADR 0016, `docs/design/configuration.md`).
//!
//! Global config (the default vault) lives in the OS-native config directory;
//! per-vault config (the view definitions) lives under the vault's `.ntropy/`
//! so it travels with the vault. Both are TOML and share one [`ConfigError`].

pub mod global;
pub mod per_vault;

use std::path::PathBuf;

pub use global::GlobalConfig;
pub use per_vault::{PerVaultConfig, ViewConfig};

/// A failure reading, parsing or writing a configuration file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("while reading config `{}`", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("while writing config `{}`", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML in config `{}`", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("could not serialize config")]
    Serialize(#[from] toml::ser::Error),
}
