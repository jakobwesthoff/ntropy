// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The vault-info use case (ADR 0018).
//!
//! Gathers the headless statistics behind the `info` command: how many notes,
//! distinct tags, views and templates a vault holds, how many notes were skipped
//! with warnings, the creation-date span, and the most-used tags. Vault
//! resolution and its source are the binary's concern and are not computed here.

use std::collections::BTreeMap;
use std::path::Path;

use super::tags::TagCount;
use crate::config::PerVaultConfig;
use crate::error::Result;
use crate::scan;
use crate::vault::Vault;

/// A summary of a vault's contents.
#[derive(Debug, Default)]
pub struct VaultStats {
    /// Number of valid notes.
    pub notes: usize,
    /// Number of distinct tags across all notes.
    pub distinct_tags: usize,
    /// Number of configured views.
    pub views: usize,
    /// Template names (the `*.md` stems in the templates dir), sorted.
    pub templates: Vec<String>,
    /// Number of files skipped with a scan warning.
    pub warnings: usize,
    /// Creation date of the oldest note, if any.
    pub oldest_date: Option<String>,
    /// Creation date of the newest note, if any.
    pub newest_date: Option<String>,
    /// The most-used tags, highest count first (up to the requested limit).
    pub top_tags: Vec<TagCount>,
}

/// Collect statistics for `vault`, keeping at most `top_n` most-used tags.
pub fn vault_stats(vault: &Vault, top_n: usize) -> Result<VaultStats> {
    let layout = vault.layout();

    // Notes and warnings come from a single scan; a vault without an
    // `all-notes/` directory yet simply has none.
    let all_notes = layout.all_notes();
    let (notes, warnings) = if all_notes.is_dir() {
        let scan = scan::scan_notes_dir(&all_notes)?;
        (scan.notes, scan.warnings)
    } else {
        (Vec::new(), Vec::new())
    };

    // Tag counts: one note counts once per distinct tag (matching `tags`).
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for note in &notes {
        for tag in &note.tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    let distinct_tags = counts.len();

    // Top tags: highest count first, ties broken alphabetically.
    let mut top_tags: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();
    top_tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    top_tags.truncate(top_n);

    // The scan is newest-first, so the ends of the list are the date span.
    let newest_date = notes.first().map(|n| n.created_date()).transpose()?;
    let oldest_date = notes.last().map(|n| n.created_date()).transpose()?;

    let views = PerVaultConfig::load(&layout.config_file())?.views.len();
    let templates = template_names(&layout.templates_dir());

    Ok(VaultStats {
        notes: notes.len(),
        distinct_tags,
        views,
        templates,
        warnings: warnings.len(),
        oldest_date,
        newest_date,
        top_tags,
    })
}

/// The sorted stems of `*.md` files directly inside `templates_dir` (empty when
/// it is absent), e.g. `default.md` -> `default`.
fn template_names(templates_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(templates_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .filter_map(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ViewConfig;

    fn vault_with_view() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy/templates")).expect("templates");

        let mut config = PerVaultConfig::default();
        config.add(ViewConfig {
            name: "by-tag".into(),
            field: "tags".into(),
        });
        std::fs::write(
            root.join(".ntropy/config.toml"),
            config.to_toml().expect("toml"),
        )
        .expect("write config");

        let vault = Vault::new(root);
        (dir, vault)
    }

    fn write(vault: &Vault, ulid: &str, content: &str) {
        std::fs::write(
            vault.layout().all_notes().join(format!("{ulid}-n.md")),
            content,
        )
        .expect("write note");
    }

    #[test]
    fn counts_and_ranks() {
        let (_g, vault) = vault_with_view();
        std::fs::write(vault.layout().default_template(), "x").expect("default template");
        std::fs::write(vault.layout().today_template(), "x").expect("today template");
        write(
            &vault,
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntitle: A\ntags: [area/work, daily]\n---\n",
        );
        write(
            &vault,
            "01BRZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntitle: B\ntags: [area/work]\n---\n",
        );

        let stats = vault_stats(&vault, 5).expect("stats");
        assert_eq!(stats.notes, 2);
        assert_eq!(stats.distinct_tags, 2);
        assert_eq!(stats.views, 1);
        assert_eq!(stats.templates, vec!["default", "today"]);
        assert_eq!(stats.warnings, 0);
        // area/work (2) ranks before daily (1).
        assert_eq!(stats.top_tags[0].tag, "area/work");
        assert_eq!(stats.top_tags[0].count, 2);
        assert_eq!(stats.top_tags[1].tag, "daily");
        assert!(stats.oldest_date.is_some());
        assert!(stats.newest_date.is_some());
    }

    #[test]
    fn top_tags_is_capped() {
        let (_g, vault) = vault_with_view();
        write(
            &vault,
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntitle: A\ntags: [a, b, c, d]\n---\n",
        );
        let stats = vault_stats(&vault, 2).expect("stats");
        assert_eq!(stats.distinct_tags, 4);
        assert_eq!(stats.top_tags.len(), 2);
    }

    #[test]
    fn empty_vault_has_no_dates_or_tags() {
        let (_g, vault) = vault_with_view();
        let stats = vault_stats(&vault, 5).expect("stats");
        assert_eq!(stats.notes, 0);
        assert_eq!(stats.distinct_tags, 0);
        assert!(stats.top_tags.is_empty());
        assert!(stats.oldest_date.is_none());
        assert!(stats.newest_date.is_none());
    }

    #[test]
    fn counts_malformed_as_warnings() {
        let (_g, vault) = vault_with_view();
        write(
            &vault,
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntitle: Ok\n---\n",
        );
        // Missing title: skipped with a warning.
        write(
            &vault,
            "01BRZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntags: [x]\n---\n",
        );
        let stats = vault_stats(&vault, 5).expect("stats");
        assert_eq!(stats.notes, 1);
        assert_eq!(stats.warnings, 1);
    }
}
