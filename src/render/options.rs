// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Render options: engine-independent output settings from the vault config.
//!
//! The options travel from the `[render]` section of the vault's
//! `config.toml` into the engines at registry construction; each engine
//! decides how to honor a setting for the formats it produces. The option
//! types are deliberately narrow enums rather than free-form strings, so a
//! typo in the config is a parse error naming the bad value instead of a
//! failed render later.

use serde::{Deserialize, Serialize};

/// The paper formats a render can target.
///
/// The curated set covers the document sizes in common use worldwide: the ISO
/// A series sizes notes realistically print on, the two book/notebook B5
/// variants (ISO for the book trade, JIS for Japan), and the North and Latin
/// American formats. The serialized names match Typst's paper identifiers,
/// so the typst engine passes them through verbatim; other engines map them
/// as they see fit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Paper {
    A3,
    #[default]
    A4,
    A5,
    IsoB5,
    JisB5,
    UsLetter,
    UsLegal,
    UsTabloid,
    UsExecutive,
    UsOficio,
}

impl Paper {
    /// The canonical kebab-case name, identical to the serialized config
    /// value and to Typst's paper identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Paper::A3 => "a3",
            Paper::A4 => "a4",
            Paper::A5 => "a5",
            Paper::IsoB5 => "iso-b5",
            Paper::JisB5 => "jis-b5",
            Paper::UsLetter => "us-letter",
            Paper::UsLegal => "us-legal",
            Paper::UsTabloid => "us-tabloid",
            Paper::UsExecutive => "us-executive",
            Paper::UsOficio => "us-oficio",
        }
    }
}

/// Engine-independent render settings, the `[render]` section of the vault
/// config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// The paper format artifacts are laid out for.
    #[serde(default)]
    pub paper: Paper,
    /// The vault's default theme name, resolved against
    /// `<vault>/.ntropy/themes/<name>.typ` (ADR 0045). `None` — the key
    /// absent — is the engine's built-in look, and so is the reserved name
    /// `default`.
    ///
    /// A free-form string rather than a narrow enum like [`Paper`], because
    /// the legal values are whatever files the vault holds; a name that
    /// resolves to nothing is reported when the theme is loaded, naming the
    /// path it looked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

impl RenderOptions {
    /// Whether every option carries its default, so serialization can omit
    /// an entirely-default `[render]` section.
    pub fn is_default(&self) -> bool {
        *self == RenderOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant round-trips through its serialized name, and that name
    /// equals `as_str`, which ties the config surface to the Typst paper
    /// identifiers.
    #[test]
    fn paper_names_round_trip_and_match_as_str() {
        let variants = [
            Paper::A3,
            Paper::A4,
            Paper::A5,
            Paper::IsoB5,
            Paper::JisB5,
            Paper::UsLetter,
            Paper::UsLegal,
            Paper::UsTabloid,
            Paper::UsExecutive,
            Paper::UsOficio,
        ];
        for paper in variants {
            let options = RenderOptions { paper, theme: None };
            let toml = toml::to_string(&options).expect("options serialize");
            assert_eq!(toml.trim(), format!("paper = \"{}\"", paper.as_str()));
            let back: RenderOptions = toml::from_str(&toml).expect("the name parses back");
            assert_eq!(back.paper, paper);
        }
    }

    #[test]
    fn paper_defaults_to_a4() {
        assert_eq!(Paper::default(), Paper::A4);
        assert_eq!(RenderOptions::default().paper, Paper::A4);
    }

    #[test]
    fn unknown_paper_name_is_a_parse_error() {
        let err = toml::from_str::<RenderOptions>("paper = \"no-such-paper\"")
            .expect_err("an unknown paper name does not parse");
        let message = err.to_string();
        assert!(
            message.contains("no-such-paper") || message.contains("unknown variant"),
            "the error names the problem: {message}"
        );
    }

    #[test]
    fn options_with_defaults_report_default() {
        assert!(RenderOptions::default().is_default());
        assert!(
            !RenderOptions {
                paper: Paper::UsLetter,
                theme: None,
            }
            .is_default()
        );
        assert!(
            !RenderOptions {
                paper: Paper::A4,
                theme: Some("corporate".into()),
            }
            .is_default(),
            "a configured theme alone is not the default section"
        );
    }

    #[test]
    fn the_theme_name_round_trips_and_is_omitted_when_absent() {
        // The key is free-form, so any name the vault holds a file for
        // survives; an absent theme writes no key at all, leaving configs that
        // never touch theming untouched.
        let options = RenderOptions {
            paper: Paper::A4,
            theme: Some("corporate".into()),
        };
        let toml = toml::to_string(&options).expect("options serialize");
        assert!(
            toml.contains(r#"theme = "corporate""#),
            "the theme name is not in the serialized form: {toml}"
        );
        let back: RenderOptions = toml::from_str(&toml).expect("the options parse back");
        assert_eq!(back, options);

        let bare = toml::to_string(&RenderOptions::default()).expect("defaults serialize");
        assert!(
            !bare.contains("theme"),
            "an absent theme still wrote a key: {bare}"
        );
    }

    #[test]
    fn a_config_without_a_theme_key_parses_as_no_theme() {
        let options: RenderOptions = toml::from_str(r#"paper = "a4""#).expect("parses");
        assert_eq!(options.theme, None);
    }
}
