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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// The paper format artifacts are laid out for.
    #[serde(default)]
    pub paper: Paper,
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
            let options = RenderOptions { paper };
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
                paper: Paper::UsLetter
            }
            .is_default()
        );
    }
}
