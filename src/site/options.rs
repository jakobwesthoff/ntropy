// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `[site]` table of the vault config (ADR 0054, ADR 0048, ADR 0056).

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
    /// Whether note pages end with their related notes (ADR 0056); on
    /// when absent. A note's `site.related` overrides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related: Option<bool>,
    /// The page the sidebar is rooted at, `tags/<path>` or
    /// `views/<name>/<group>` (ADR 0056); the vault root when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The hand-assembled sidebar, the `[[site.nav]]` tables (ADR 0056);
    /// when present it is the whole sidebar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav: Vec<NavSection>,
}

/// One section of the hand-assembled sidebar, a `[[site.nav]]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavSection {
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<NavEntry>,
}

/// One item of a nav section as written in the config: exactly one of
/// `note`, `tag`, `view`, `tags`, or `items` says what it is, `group`
/// narrows a view to one of its groups, and `label` renames the item.
/// [`NavEntry::kind`] checks the combination.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<NavEntry>>,
}

/// What a nav entry names, once its keys are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavEntryKind<'a> {
    /// A note, by ULID.
    Note(&'a str),
    /// A tag's subtree.
    Tag(&'a str),
    /// A view, or one group of it.
    View {
        name: &'a str,
        group: Option<&'a str>,
    },
    /// The whole tag tree.
    Tags,
    /// A group assembled by hand.
    Group(&'a [NavEntry]),
}

impl NavEntry {
    /// The entry's kind, or why it has none: no naming key, more than one,
    /// `group` without `view`, or `tags = false`.
    pub fn kind(&self) -> Result<NavEntryKind<'_>, String> {
        let mut kinds: Vec<NavEntryKind<'_>> = Vec::new();
        if let Some(note) = &self.note {
            kinds.push(NavEntryKind::Note(note));
        }
        if let Some(tag) = &self.tag {
            kinds.push(NavEntryKind::Tag(tag));
        }
        if let Some(view) = &self.view {
            kinds.push(NavEntryKind::View {
                name: view,
                group: self.group.as_deref(),
            });
        }
        match self.tags {
            Some(true) => kinds.push(NavEntryKind::Tags),
            Some(false) => return Err("`tags` can only be true".to_string()),
            None => {}
        }
        if let Some(items) = &self.items {
            kinds.push(NavEntryKind::Group(items));
        }
        if self.group.is_some() && self.view.is_none() {
            return Err("`group` needs a `view`".to_string());
        }
        match kinds.len() {
            1 => Ok(kinds.remove(0)),
            0 => Err("an item needs one of `note`, `tag`, `view`, `tags`, or `items`".to_string()),
            _ => Err(
                "an item takes only one of `note`, `tag`, `view`, `tags`, or `items`".to_string(),
            ),
        }
    }
}

impl SiteOptions {
    pub fn is_default(&self) -> bool {
        *self == SiteOptions::default()
    }

    /// The page language, configured or default.
    pub fn lang(&self) -> &str {
        self.lang.as_deref().unwrap_or(DEFAULT_LANG)
    }

    /// Whether note pages list their related notes unless a note says
    /// otherwise.
    pub fn related(&self) -> bool {
        self.related.unwrap_or(true)
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
            "theme = \"corporate\"\nindex = \"01ARZ3NDEKTSV4RRFFQ69G5FAV\"\ntitle = \"Docs\"\nlang = \"de\"\nrelated = false\n",
        )
        .expect("parse");
        assert!(!options.related());
        assert!(SiteOptions::default().related());
        assert_eq!(options.theme.as_deref(), Some("corporate"));
        assert_eq!(options.index.as_deref(), Some("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
        assert_eq!(options.title.as_deref(), Some("Docs"));
        assert_eq!(options.lang(), "de");
        assert!(!options.is_default());
    }

    #[test]
    fn the_root_and_the_nav_table_parse_and_round_trip() {
        let text = "root = \"tags/docs\"\n\n[[nav]]\nlabel = \"Getting Started\"\nitems = [\n  { note = \"01ARZ3NDEKTSV4RRFFQ69G5FAV\" },\n  { label = \"Gadgets\", tag = \"docs/start/gadgets\" },\n  { view = \"by-status\", group = \"open\" },\n  { tags = true },\n  { label = \"More\", items = [{ view = \"by-status\" }] },\n]\n\n[[nav]]\nlabel = \"Empty\"\n";
        let options: SiteOptions = toml::from_str(text).expect("parse");
        assert_eq!(options.root.as_deref(), Some("tags/docs"));
        assert_eq!(options.nav.len(), 2);
        let first = &options.nav[0];
        assert_eq!(first.label, "Getting Started");
        let kinds: Vec<NavEntryKind<'_>> = first
            .items
            .iter()
            .map(|item| item.kind().expect("a valid item"))
            .collect();
        assert_eq!(
            kinds,
            [
                NavEntryKind::Note("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
                NavEntryKind::Tag("docs/start/gadgets"),
                NavEntryKind::View {
                    name: "by-status",
                    group: Some("open"),
                },
                NavEntryKind::Tags,
                NavEntryKind::Group(&options.nav[0].items[4].items.clone().expect("items")),
            ]
        );
        assert_eq!(first.items[1].label.as_deref(), Some("Gadgets"));
        assert!(options.nav[1].items.is_empty());
        let written = toml::to_string(&options).expect("serialize");
        let back: SiteOptions = toml::from_str(&written).expect("parse back");
        assert_eq!(back, options);
    }

    #[test]
    fn a_nav_item_with_an_unknown_key_does_not_parse() {
        let error = toml::from_str::<SiteOptions>(
            "[[nav]]\nlabel = \"X\"\nitems = [{ notes = \"01ARZ3NDEKTSV4RRFFQ69G5FAV\" }]\n",
        )
        .expect_err("an unknown key is refused");
        assert!(error.to_string().contains("notes"), "{error}");
    }

    #[test]
    fn a_nav_item_names_exactly_one_thing() {
        let entry = |text: &str| toml::from_str::<NavEntry>(text).expect("parse");
        assert!(
            entry("")
                .kind()
                .expect_err("empty")
                .contains("needs one of")
        );
        assert!(
            entry("note = \"x\"\ntag = \"y\"\n")
                .kind()
                .expect_err("two")
                .contains("only one of")
        );
        assert!(
            entry("group = \"open\"\n")
                .kind()
                .expect_err("group alone")
                .contains("needs a `view`")
        );
        assert!(
            entry("tags = false\n")
                .kind()
                .expect_err("tags false")
                .contains("only be true")
        );
        assert_eq!(
            entry("view = \"v\"\n").kind().expect("view"),
            NavEntryKind::View {
                name: "v",
                group: None
            }
        );
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
