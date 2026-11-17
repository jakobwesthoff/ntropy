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

/// The default per-file limit for attaching a linked file to a PDF, in bytes.
pub const DEFAULT_ATTACH_MAX: u64 = 25 * 1024 * 1024;

/// Engine-independent render settings, the `[render]` section of the vault
/// config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// The paper format artifacts are laid out for.
    #[serde(default)]
    pub paper: Paper,
    /// The vault's default theme name, resolved against
    /// `<vault>/.ntropy/themes/typst/<name>.typ` (ADR 0045, ADR 0047).
    /// `None` — the key
    /// absent — is the engine's built-in look, and so is the reserved name
    /// `default`.
    ///
    /// A free-form string rather than a narrow enum like [`Paper`], because
    /// the legal values are whatever files the vault holds; a name that
    /// resolves to nothing is reported when the theme is loaded, naming the
    /// path it looked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The per-file size limit in bytes up to which a linked file is attached
    /// to a PDF; `0` turns attaching off.
    ///
    /// The config takes an integer (bytes) or a size string such as
    /// `"25 MiB"`; the default is left out when the section is written.
    #[serde(
        default = "default_attach_max",
        deserialize_with = "attach_max_from_toml",
        skip_serializing_if = "is_default_attach_max"
    )]
    pub attach_max: u64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            paper: Paper::default(),
            theme: None,
            attach_max: DEFAULT_ATTACH_MAX,
        }
    }
}

fn default_attach_max() -> u64 {
    DEFAULT_ATTACH_MAX
}

fn is_default_attach_max(bytes: &u64) -> bool {
    *bytes == DEFAULT_ATTACH_MAX
}

/// Reads `attach_max` as an integer byte count or a size string. A hand-written
/// visitor rather than an untagged enum, so the error for a bad value names
/// the value instead of "data did not match any variant".
fn attach_max_from_toml<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected, Visitor};

    struct SizeVisitor;

    impl Visitor<'_> for SizeVisitor {
        type Value = u64;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a size in bytes or a size string such as \"25 MiB\"")
        }

        fn visit_i64<E: Error>(self, bytes: i64) -> Result<u64, E> {
            u64::try_from(bytes).map_err(|_| E::invalid_value(Unexpected::Signed(bytes), &self))
        }

        fn visit_str<E: Error>(self, text: &str) -> Result<u64, E> {
            crate::asset::parse_size(text)
                .ok_or_else(|| E::invalid_value(Unexpected::Str(text), &self))
        }
    }

    deserializer.deserialize_any(SizeVisitor)
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
            let options = RenderOptions {
                paper,
                theme: None,
                ..Default::default()
            };
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
                ..Default::default()
            }
            .is_default()
        );
        assert!(
            !RenderOptions {
                paper: Paper::A4,
                theme: Some("corporate".into()),
                ..Default::default()
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
            ..Default::default()
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

    #[test]
    fn attach_max_defaults_to_25_mib() {
        assert_eq!(DEFAULT_ATTACH_MAX, 25 * 1024 * 1024);
        assert_eq!(RenderOptions::default().attach_max, DEFAULT_ATTACH_MAX);
        let options: RenderOptions = toml::from_str("").expect("parses");
        assert_eq!(options.attach_max, DEFAULT_ATTACH_MAX);
    }

    #[test]
    fn attach_max_accepts_integer_bytes() {
        let options: RenderOptions = toml::from_str("attach_max = 1048576").expect("parses");
        assert_eq!(options.attach_max, 1024 * 1024);
    }

    #[test]
    fn attach_max_accepts_a_size_string() {
        let options: RenderOptions = toml::from_str(r#"attach_max = "512 KiB""#).expect("parses");
        assert_eq!(options.attach_max, 512 * 1024);
    }

    #[test]
    fn attach_max_zero_turns_attaching_off_as_integer_and_as_string() {
        for text in [
            "attach_max = 0",
            r#"attach_max = "0""#,
            r#"attach_max = "0 B""#,
        ] {
            let options: RenderOptions = toml::from_str(text).expect("parses");
            assert_eq!(options.attach_max, 0, "{text}");
        }
    }

    #[test]
    fn a_bad_attach_max_error_names_the_key_and_the_value() {
        for (text, value) in [
            (r#"attach_max = "lots""#, "lots"),
            ("attach_max = -5", "-5"),
            ("attach_max = 1.5", "1.5"),
        ] {
            let message = toml::from_str::<RenderOptions>(text)
                .expect_err("not a size")
                .to_string();
            assert!(
                message.contains("attach_max") && message.contains(value),
                "the error for {text} names key and value: {message}"
            );
        }
    }

    #[test]
    fn attach_max_is_omitted_at_its_default_and_written_as_bytes_otherwise() {
        let bare = toml::to_string(&RenderOptions::default()).expect("defaults serialize");
        assert!(
            !bare.contains("attach_max"),
            "the default was written: {bare}"
        );

        for (attach_max, line) in [(1024 * 1024, "attach_max = 1048576"), (0, "attach_max = 0")] {
            let options = RenderOptions {
                attach_max,
                ..Default::default()
            };
            let text = toml::to_string(&options).expect("serializes");
            assert!(text.contains(line), "{text}");
            let back: RenderOptions = toml::from_str(&text).expect("parses back");
            assert_eq!(back, options);
        }
    }

    #[test]
    fn a_changed_attach_max_is_not_the_default_section() {
        let options = RenderOptions {
            attach_max: 0,
            ..Default::default()
        };
        assert!(!options.is_default());
    }
}
