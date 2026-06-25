// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The tags use case (ADR 0018).
//!
//! Lists every distinct full tag string across the vault with the number of
//! notes carrying it, sorted alphabetically. Tags are the normalized full
//! forms (e.g. `area/work`), already de-duplicated per note by the note model,
//! so a note counts once per distinct tag.

use std::collections::BTreeMap;

use crate::error::Result;
use crate::scan::{self, ScanWarning};
use crate::vault::Vault;

/// One tag and the number of notes carrying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}

/// The result of listing tags: counts (alphabetical) plus scan warnings.
#[derive(Debug, Default)]
pub struct TagList {
    pub tags: Vec<TagCount>,
    pub warnings: Vec<ScanWarning>,
}

/// List all tags with their note counts, sorted alphabetically.
pub fn list_tags(vault: &Vault) -> Result<TagList> {
    let scan = scan::scan_notes_dir(&vault.layout().all_notes())?;

    // A `BTreeMap` gives alphabetical order for free.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for note in &scan.notes {
        for tag in &note.tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    let tags = counts
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();

    Ok(TagList {
        tags,
        warnings: scan.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let vault = Vault::new(dir.path());
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
    fn counts_and_sorts_tags() {
        let (_g, v) = temp_vault();
        write(
            &v,
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntitle: A\ntags: [area/work, programming/rust]\n---\n",
        );
        write(
            &v,
            "01BRZ3NDEKTSV4RRFFQ69G5FAV",
            "---\ntitle: B\ntags: [area/work]\n---\n",
        );

        let list = list_tags(&v).expect("tags");
        assert_eq!(
            list.tags,
            vec![
                TagCount {
                    tag: "area/work".into(),
                    count: 2
                },
                TagCount {
                    tag: "programming/rust".into(),
                    count: 1
                },
            ]
        );
    }

    #[test]
    fn empty_vault_has_no_tags() {
        let (_g, v) = temp_vault();
        assert!(list_tags(&v).expect("tags").tags.is_empty());
    }
}
