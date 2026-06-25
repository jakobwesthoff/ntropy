// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Integration tests over a temporary vault, snapshotting the materialized
//! symlink trees and their relative link targets (ADR 0021).
//!
//! Leaf names embed the creation date, which depends on the host timezone, so
//! every snapshot redacts `YYYY-MM-DD` to `[DATE]`. The link targets and tree
//! structure (the parts under test) stay deterministic because the notes use
//! fixed ULIDs.

use std::path::{Path, PathBuf};

use ntropy::config::{PerVaultConfig, ViewConfig};
use ntropy::reconcile;
use ntropy::vault::Vault;

/// Two ULIDs sharing a timestamp prefix (so they render the same date) but
/// differing in their random tail, used to force a leaf-name collision.
const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_B: &str = "01ARZ3NDEKTSV4RRFFQ69G5FBW";
const ULID_C: &str = "01BRZ3NDEKTSV4RRFFQ69G5FCX";

/// Create a vault with `all-notes/` and the given views configured.
fn vault_with_views(views: &[(&str, &str)]) -> (tempfile::TempDir, Vault) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");

    let mut config = PerVaultConfig::default();
    for (name, field) in views {
        config.add(ViewConfig {
            name: (*name).into(),
            field: (*field).into(),
        });
    }
    std::fs::write(
        root.join(".ntropy").join("config.toml"),
        config.to_toml().expect("toml"),
    )
    .expect("write config");

    let vault = Vault::new(root);
    (dir, vault)
}

fn write_note(vault: &Vault, ulid: &str, slug: &str, content: &str) {
    let path = vault.layout().all_notes().join(format!("{ulid}-{slug}.md"));
    std::fs::write(path, content).expect("write note");
}

/// Render every symlink under `dir` as a sorted `relative -> target` listing.
fn list_tree(dir: &Path) -> String {
    let mut entries = Vec::new();
    collect_symlinks(dir, dir, &mut entries);
    entries.sort();
    entries.join("\n")
}

fn collect_symlinks(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read {
        let path = entry.expect("entry").path();
        let meta = std::fs::symlink_metadata(&path).expect("symlink metadata");
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&path).expect("read_link");
            let rel = path.strip_prefix(base).expect("relative");
            out.push(format!("{} -> {}", rel.display(), target.display()));
        } else if meta.is_dir() {
            collect_symlinks(base, &path, out);
        }
    }
}

/// Snapshot a tree with the date portion of leaf names redacted.
macro_rules! assert_tree_snapshot {
    ($tree:expr) => {
        insta::with_settings!({filters => vec![(r"\d{4}-\d{2}-\d{2}", "[DATE]")]}, {
            insta::assert_snapshot!($tree);
        });
    };
}

#[test]
fn tags_view_nests_and_fans_out() {
    let (_guard, vault) = vault_with_views(&[("by-tag", "tags")]);
    write_note(
        &vault,
        ULID_A,
        "quarterly-review",
        "---\ntitle: Quarterly Review\ntags: [programming/rust, area/work]\n---\nbody\n",
    );

    reconcile::reconcile(&vault).expect("reconcile");

    let tree = list_tree(&vault.root().join("by-tag"));
    assert_tree_snapshot!(tree);
}

#[test]
fn leaf_collisions_are_disambiguated() {
    let (_guard, vault) = vault_with_views(&[("by-tag", "tags")]);
    // Same title and same date (shared ULID timestamp prefix) under one tag.
    write_note(
        &vault,
        ULID_A,
        "review",
        "---\ntitle: Review\ntags: [work]\n---\nbody\n",
    );
    write_note(
        &vault,
        ULID_B,
        "review",
        "---\ntitle: Review\ntags: [work]\n---\nbody\n",
    );

    reconcile::reconcile(&vault).expect("reconcile");

    let tree = list_tree(&vault.root().join("by-tag"));
    assert_tree_snapshot!(tree);
}

#[test]
fn notes_without_the_field_are_skipped() {
    let (_guard, vault) = vault_with_views(&[("by-status", "status")]);
    write_note(
        &vault,
        ULID_A,
        "has-status",
        "---\ntitle: Has Status\nstatus: In Progress\n---\nbody\n",
    );
    write_note(
        &vault,
        ULID_C,
        "no-status",
        "---\ntitle: No Status\n---\nbody\n",
    );

    reconcile::reconcile(&vault).expect("reconcile");

    let tree = list_tree(&vault.root().join("by-status"));
    // Only the note carrying `status` appears; grouping value is normalized.
    assert_tree_snapshot!(tree);
}

#[test]
fn rebuild_catches_up_after_out_of_band_edit() {
    let (_guard, vault) = vault_with_views(&[("by-tag", "tags")]);
    let path: PathBuf = vault.layout().all_notes().join(format!("{ULID_A}-note.md"));
    std::fs::write(&path, "---\ntitle: Note\ntags: [old]\n---\nbody\n").expect("write");
    reconcile::reconcile(&vault).expect("first reconcile");
    assert!(vault.root().join("by-tag/old").is_dir());

    // Edit the tag out of band, then reconcile: the old group is pruned and the
    // new one appears.
    std::fs::write(&path, "---\ntitle: Note\ntags: [new]\n---\nbody\n").expect("rewrite");
    reconcile::reconcile(&vault).expect("second reconcile");

    assert!(!vault.root().join("by-tag/old").exists());
    let tree = list_tree(&vault.root().join("by-tag"));
    assert_tree_snapshot!(tree);
}
