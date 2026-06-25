// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The per-session note cache (ADR 0029).
//!
//! Completion, navigation and symbol lookup all read note metadata. Rather than
//! scan the vault on every keystroke, the server keeps an in-memory projection
//! of each vault it has touched, keyed by the canonicalized vault root. It is
//! populated lazily on first use and refreshed by a full rescan when the client
//! reports a file change (or, as a fallback, when a document is opened). The
//! cache is process-local and ephemeral, so it is a session cache rather than a
//! persisted index (it does not contradict ADR 0002).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ntropy::id::Id;
use ntropy::scan;
use ntropy::vault::Vault;

/// A note's cached metadata: everything completion and navigation need without
/// re-reading the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub id: Id,
    pub title: String,
    pub tags: Vec<String>,
    pub path: PathBuf,
}

/// Lazily-populated note metadata, one bucket per vault root.
#[derive(Debug, Default)]
pub struct Cache {
    by_root: HashMap<PathBuf, Vec<CacheEntry>>,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached entries for a vault, scanning it on first use.
    ///
    /// Entries keep the scan's newest-first order (ADR 0025). Malformed notes
    /// are skipped silently, as the server has no `--strict` channel to report
    /// them.
    pub fn entries(&mut self, vault: &Vault) -> &[CacheEntry] {
        let root = vault.root().to_path_buf();
        self.by_root
            .entry(root)
            .or_insert_with(|| scan_entries(vault))
    }

    /// Every cached entry across all populated vault roots.
    ///
    /// Used by workspace symbols, which have no document context, so they range
    /// over whatever vaults the session has already touched.
    pub fn all_entries(&self) -> Vec<&CacheEntry> {
        self.by_root.values().flatten().collect()
    }

    /// Drop a vault's cached entries so the next access rescans it.
    pub fn invalidate(&mut self, root: &Path) {
        self.by_root.remove(root);
    }

    /// Whether a vault root currently has cached entries (test seam).
    #[cfg(test)]
    pub fn is_populated(&self, root: &Path) -> bool {
        self.by_root.contains_key(root)
    }
}

/// Scan a vault's `all-notes/` into cache entries, or an empty set on error.
fn scan_entries(vault: &Vault) -> Vec<CacheEntry> {
    let Ok(scan) = scan::scan_notes_dir(&vault.layout().all_notes()) else {
        return Vec::new();
    };
    scan.notes
        .into_iter()
        .map(|note| CacheEntry {
            id: note.id,
            title: note.title,
            tags: note.tags,
            path: note.path,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn vault_with(notes: &[(&str, &str, &str)]) -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        for (ulid, slug, content) in notes {
            std::fs::write(
                root.join("all-notes").join(format!("{ulid}-{slug}.md")),
                content,
            )
            .expect("write note");
        }
        let vault = Vault::new(std::fs::canonicalize(root).expect("canonicalize"));
        (dir, vault)
    }

    #[test]
    fn populates_lazily_on_first_access() {
        let (_guard, vault) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let mut cache = Cache::new();
        assert!(!cache.is_populated(vault.root()));
        assert_eq!(cache.entries(&vault).len(), 1);
        assert!(cache.is_populated(vault.root()));
        assert_eq!(cache.entries(&vault)[0].title, "Alpha");
    }

    #[test]
    fn does_not_rescan_until_invalidated() {
        let (_guard, vault) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let mut cache = Cache::new();
        assert_eq!(cache.entries(&vault).len(), 1);

        // A note added out of band is invisible while the cache holds.
        std::fs::write(
            vault.layout().all_notes().join(format!("{ULID_B}-b.md")),
            "---\ntitle: Beta\n---\n",
        )
        .expect("write note");
        assert_eq!(cache.entries(&vault).len(), 1);

        // After invalidation the rescan sees it.
        cache.invalidate(vault.root());
        assert_eq!(cache.entries(&vault).len(), 2);
    }

    #[test]
    fn empty_vault_yields_no_entries() {
        let (_guard, vault) = vault_with(&[]);
        let mut cache = Cache::new();
        assert!(cache.entries(&vault).is_empty());
    }

    #[test]
    fn malformed_notes_are_excluded() {
        let (_guard, vault) = vault_with(&[
            (ULID_A, "a", "---\ntitle: Good\n---\n"),
            // Missing `title` is malformed and skipped by the scan.
            (ULID_B, "b", "---\ntags: [x]\n---\n"),
        ]);
        let mut cache = Cache::new();
        assert_eq!(cache.entries(&vault).len(), 1);
    }

    #[test]
    fn distinct_roots_are_cached_independently() {
        let (_a, vault_a) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let (_b, vault_b) = vault_with(&[
            (ULID_A, "x", "---\ntitle: X\n---\n"),
            (ULID_B, "y", "---\ntitle: Y\n---\n"),
        ]);
        let mut cache = Cache::new();
        assert_eq!(cache.entries(&vault_a).len(), 1);
        assert_eq!(cache.entries(&vault_b).len(), 2);
    }

    #[test]
    fn all_entries_spans_every_populated_root() {
        let (_a, vault_a) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let (_b, vault_b) = vault_with(&[(ULID_B, "b", "---\ntitle: Beta\n---\n")]);
        let mut cache = Cache::new();
        assert!(cache.all_entries().is_empty());
        cache.entries(&vault_a);
        cache.entries(&vault_b);
        assert_eq!(cache.all_entries().len(), 2);
    }
}
