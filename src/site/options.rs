// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `[site]` table of the vault config (ADR 0054, ADR 0048).

use serde::{Deserialize, Serialize};

/// The `lang` attribute a page carries when the vault configures none.
pub const DEFAULT_LANG: &str = "en";

/// The site settings of a vault, the `[site]` table in
/// `<vault>/.ntropy/config.toml`. Every key is optional; an absent table is
/// the default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteOptions {
    /// The site theme, a directory `<vault>/.ntropy/themes/site/<name>/`.
    /// `None`, and the reserved name `default`, mean the built-in theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The ULID of the note that becomes the front page; without it the
    /// export generates an overview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// The site title; the vault directory name when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The `lang` attribute of every page; [`DEFAULT_LANG`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

impl SiteOptions {
    pub fn is_default(&self) -> bool {
        *self == SiteOptions::default()
    }

    /// The page language, configured or default.
    pub fn lang(&self) -> &str {
        self.lang.as_deref().unwrap_or(DEFAULT_LANG)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_table_is_the_default() {
        let options: SiteOptions = toml::from_str("").expect("parse");
        assert!(options.is_default());
        assert_eq!(options.lang(), "en");
    }

    #[test]
    fn every_key_parses() {
        let options: SiteOptions = toml::from_str(
            "theme = \"corporate\"\nindex = \"01ARZ3NDEKTSV4RRFFQ69G5FAV\"\ntitle = \"Docs\"\nlang = \"de\"\n",
        )
        .expect("parse");
        assert_eq!(options.theme.as_deref(), Some("corporate"));
        assert_eq!(options.index.as_deref(), Some("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
        assert_eq!(options.title.as_deref(), Some("Docs"));
        assert_eq!(options.lang(), "de");
        assert!(!options.is_default());
    }

    #[test]
    fn only_set_keys_serialize() {
        let options = SiteOptions {
            theme: Some("corporate".to_string()),
            ..SiteOptions::default()
        };
        assert_eq!(
            toml::to_string(&options).expect("serialize"),
            "theme = \"corporate\"\n"
        );
    }
}
