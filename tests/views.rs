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
use ntropy::session::VaultSession;
use ntropy::vault::Vault;

/// Two ULIDs sharing a timestamp prefix (so they render the same date) but
/// differing in their random tail, used to force a leaf-name collision.
const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_B: &str = "01ARZ3NDEKTSV4RRFFQ69G5FBW";
const ULID_C: &str = "01BRZ3NDEKTSV4RRFFQ69G5FCX";

/// Create a vault with `all-notes/` and the given views configured.
fn vault_with_views(views: &[(&str, &str)]) -> (tempfile::TempDir, VaultSession) {
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

    let session = VaultSession::plaintext(Vault::new(root));
    (dir, session)
}

fn write_note(session: &VaultSession, ulid: &str, slug: &str, content: &str) {
    let path = session
        .layout()
        .all_notes()
        .join(format!("{ulid}-{slug}.md"));
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

// =============================================================================
// Hostile view names
// =============================================================================
//
// A view's directory is emptied of everything that is not one of its links,
// so a name that points anywhere else turns a sync into a recursive delete.
// Every test here keeps its victim tree inside its own temporary directory:
// the vault sits one level below `base`, and the decoy beside it, so even a
// regression can only delete files the test created.

/// A vault at `<base>/vault` with the given views configured, plus the
/// `base` directory that holds it.
fn nested_vault(views: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf, VaultSession) {
    let dir = tempfile::tempdir().expect("temp dir");
    let base = dir.path().to_path_buf();
    let root = base.join("vault");
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

    (dir, base, VaultSession::plaintext(Vault::new(root)))
}

/// Plant files a sync must never touch under `dir`, returning their paths.
fn plant_victims(dir: &Path) -> Vec<PathBuf> {
    let files = [
        dir.join("important.txt"),
        dir.join("sub").join("nested.txt"),
    ];
    for file in &files {
        std::fs::create_dir_all(file.parent().expect("a planted file has a parent"))
            .expect("victim dir");
        std::fs::write(file, "keep me").expect("victim file");
    }
    files.to_vec()
}

fn assert_all_exist(guard: &tempfile::TempDir, files: &[PathBuf]) {
    for file in files {
        assert!(
            file.starts_with(guard.path()),
            "the victim `{}` must live in the test's own temp dir",
            file.display()
        );
        assert!(file.exists(), "`{}` was deleted", file.display());
    }
}

#[test]
fn a_traversing_view_name_cannot_delete_outside_the_vault() {
    let (guard, base, vault) = nested_vault(&[("../decoy", "tags")]);
    let victims = plant_victims(&base.join("decoy"));

    let report = reconcile::reconcile(&vault).expect("reconcile");

    assert_all_exist(&guard, &victims);
    let skipped: Vec<&str> = report
        .skipped_views
        .iter()
        .map(|v| v.name.as_str())
        .collect();
    assert_eq!(skipped, ["../decoy"]);
}

#[test]
fn a_traversing_view_name_cannot_delete_outside_the_vault_on_refresh() {
    // `refresh_views` is what `new`, `today`, `write`, `delete` and the
    // editor flow run after every change.
    let (guard, base, vault) = nested_vault(&[("../decoy", "tags")]);
    let victims = plant_victims(&base.join("decoy"));

    reconcile::refresh_views(&vault).expect("refresh");

    assert_all_exist(&guard, &victims);
}

#[test]
fn an_absolute_view_name_cannot_delete_outside_the_vault() {
    let dir = tempfile::tempdir().expect("temp dir");
    let decoy = dir.path().join("decoy");
    let name = decoy.to_str().expect("temp paths are UTF-8").to_string();
    let (guard, _base, vault) = nested_vault(&[(&name, "tags")]);
    let victims = plant_victims(&decoy);

    reconcile::reconcile(&vault).expect("reconcile");

    // The victims live in a second temp dir, so check that one.
    assert_all_exist(&dir, &victims);
    drop(guard);
}

#[test]
fn a_dot_git_view_name_cannot_empty_the_repository() {
    let (guard, _base, vault) = nested_vault(&[(".git", "tags")]);
    let git = vault.root().join(".git");
    let victims = [
        git.join("HEAD"),
        git.join("refs").join("heads").join("main"),
    ];
    for file in &victims {
        std::fs::create_dir_all(file.parent().expect("a git file has a parent")).expect("git dir");
        std::fs::write(file, "ref: refs/heads/main\n").expect("git file");
    }

    reconcile::reconcile(&vault).expect("reconcile");

    assert_all_exist(&guard, &victims);
}

#[test]
fn a_reserved_prefix_view_name_cannot_touch_the_notes() {
    let (guard, _base, vault) = nested_vault(&[("all-notes/x", "status")]);
    write_note(
        &vault,
        ULID_A,
        "keep",
        "---\ntitle: Keep\nstatus: open\n---\nbody\n",
    );
    let resource = vault.layout().all_notes().join("x").join("keep.txt");
    std::fs::create_dir_all(resource.parent().expect("a resource has a parent"))
        .expect("resource dir");
    std::fs::write(&resource, "an attachment").expect("resource");

    reconcile::reconcile(&vault).expect("reconcile");

    assert_all_exist(&guard, &[resource]);
}

// A valid name is not enough when a symlink planted in the vault, for example
// in a cloned tree, sits on the way to the view's directory.

#[test]
fn a_symlinked_view_root_is_skipped() {
    let (guard, base, vault) = nested_vault(&[("by-tag", "tags")]);
    write_note(
        &vault,
        ULID_A,
        "note",
        "---\ntitle: Note\ntags: [work]\n---\nbody\n",
    );
    let victims = plant_victims(&base.join("decoy"));
    std::os::unix::fs::symlink(base.join("decoy"), vault.root().join("by-tag"))
        .expect("plant symlink");

    let report = reconcile::reconcile(&vault).expect("reconcile");

    assert_all_exist(&guard, &victims);
    let skipped: Vec<&str> = report
        .skipped_views
        .iter()
        .map(|v| v.name.as_str())
        .collect();
    assert_eq!(skipped, ["by-tag"]);
    assert_eq!(report.views_synced, 0);
    // Still configured and valid, so it stays ignored.
    let gitignore = std::fs::read_to_string(vault.layout().gitignore_file()).expect("read");
    assert!(gitignore.contains("/by-tag/"), "{gitignore}");
}

#[test]
fn a_symlinked_intermediate_segment_is_skipped() {
    let (guard, base, vault) = nested_vault(&[("area/work", "tags")]);
    let victims = plant_victims(&base.join("decoy").join("work"));
    std::os::unix::fs::symlink(base.join("decoy"), vault.root().join("area"))
        .expect("plant symlink");

    let refresh = reconcile::refresh_views(&vault).expect("refresh");

    assert_all_exist(&guard, &victims);
    let skipped: Vec<&str> = refresh
        .skipped_views
        .iter()
        .map(|v| v.name.as_str())
        .collect();
    assert_eq!(skipped, ["area/work"]);
}
