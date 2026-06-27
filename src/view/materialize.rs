// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Materializing one view as a symlink tree (ADRs 0008, 0009, 0023).
//!
//! A view groups notes by one frontmatter field. Each grouping value (always
//! normalized) becomes a directory path under the view, a `/` in the value
//! nests further, and a list-valued field places a note under each of its
//! values. The leaf in each group is a relative symlink back into `all-notes/`,
//! named `<date>-<slug>.md` with collisions disambiguated (see [`super::leaf`]).
//! A note with no value for the field is skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde_yaml_ng::Value;

use crate::error::Result;
use crate::fsutil;
use crate::note::Note;
use crate::text::{slug, tag};
use crate::vault::Vault;

use super::ViewDef;
use super::leaf::{self, LeafInput};

/// Existing leaf entries of a view tree keyed by path: `Some(target)` is a
/// symlink's stored target, `None` is any other file occupying that path.
type LeafMap = BTreeMap<PathBuf, Option<PathBuf>>;

/// The set of subdirectory paths within a view tree, used to prune emptied
/// groups deepest-first.
type DirSet = BTreeSet<PathBuf>;

/// Bring a single view's directory in line with the current note set.
///
/// Rather than tearing the tree down and rebuilding it, this computes the view's
/// desired projection and diffs it against what is already on disk, touching only
/// the difference: a leaf that already points where it should is left untouched,
/// so a mutation costs filesystem writes proportional to what actually changed
/// rather than to the whole vault. The resulting tree is exactly what a
/// from-scratch build would produce — stale and drifted links removed, emptied
/// group directories pruned — but unchanged links keep their identity (ADR 0008).
pub fn sync_view(vault: &Vault, view: &ViewDef, notes: &[Note]) -> Result<()> {
    let view_dir = vault.layout().view_dir(&view.name);

    let desired = desired_links(&view_dir, view, notes)?;
    let (actual, dirs) = actual_state(&view_dir)?;

    // A configured view always has a root directory, even with no matching notes.
    fsutil::create_dir_all(&view_dir)?;

    // Remove every on-disk entry that is not already a correct leaf: a stale
    // leaf, a leaf whose target drifted (removed here, recreated just below), and
    // any non-symlink file (`target` is `None`), which is never a correct leaf and
    // so is always removed — whether it sits at a desired path or a stray one.
    for (path, target) in &actual {
        let is_correct_leaf = matches!(target, Some(existing) if desired.get(path) == Some(existing));
        if !is_correct_leaf {
            fsutil::remove_file(path)?;
        }
    }

    // Create the links that are missing, or were just removed for a retarget.
    // `symlink` creates any missing parent directories.
    for (path, target) in &desired {
        let already_correct = matches!(actual.get(path), Some(Some(existing)) if existing == target);
        if !already_correct {
            fsutil::symlink(target, path)?;
        }
    }

    // Removals may have emptied group directories. Attempt every directory the
    // walk saw, deepest-first (a child path sorts after its parent), so a freshly
    // emptied child is gone before its parent is examined; `remove_dir_if_empty`
    // is a no-op on the ones still holding leaves. The view's own root is never in
    // this set, so it is always kept.
    for dir in dirs.iter().rev() {
        fsutil::remove_dir_if_empty(dir)?;
    }

    Ok(())
}

/// The full set of links a view should contain: leaf path -> stored target.
///
/// Identical grouping and leaf-naming to a from-scratch build, but it produces
/// the desired map instead of writing symlinks, so it can be diffed against disk.
fn desired_links(
    view_dir: &Path,
    view: &ViewDef,
    notes: &[Note],
) -> Result<BTreeMap<PathBuf, PathBuf>> {
    // Group notes by normalized field value. The group-by itself stays serial:
    // it is a cheap `normalize` (or a tag-list clone) per note, a small Amdahl
    // floor not worth a parallel pre-pass. Sorting here is only to keep the
    // parallel map's input order stable; the path-keyed `BTreeMap` below makes
    // the final projection deterministic regardless of completion order.
    let mut groups: BTreeMap<String, Vec<&Note>> = BTreeMap::new();
    for note in notes {
        for value in group_values(note, &view.field) {
            groups.entry(value).or_default().push(note);
        }
    }

    // Each group writes a disjoint subdirectory and disambiguates only within
    // itself, so the per-group leaf construction is embarrassingly parallel.
    // Build each group's leaves in parallel, then flatten into the path-keyed
    // map: disjoint subtrees mean keys never collide, so the merge is
    // conflict-free and the output is identical to a serial build.
    let groups: Vec<(String, Vec<&Note>)> = groups.into_iter().collect();
    let per_group: Vec<Vec<(PathBuf, PathBuf)>> = groups
        .into_par_iter()
        .map(|(value, group_notes)| group_leaves(view_dir, &value, &group_notes))
        .collect::<Result<_>>()?;

    let mut desired = BTreeMap::new();
    for leaf in per_group.into_iter().flatten() {
        desired.insert(leaf.0, leaf.1);
    }
    Ok(desired)
}

/// The leaves a single group contributes: `(leaf path, stored target)` pairs.
///
/// Disambiguation ([`leaf::leaf_names`]) is scoped to the group, so this is the
/// largest unit that can be built independently of the rest of the view.
fn group_leaves(
    view_dir: &Path,
    value: &str,
    group_notes: &[&Note],
) -> Result<Vec<(PathBuf, PathBuf)>> {
    // A value's `/` segments nest into subdirectories.
    let leaf_dir = view_dir.join(value);

    let mut inputs = Vec::with_capacity(group_notes.len());
    for note in group_notes {
        inputs.push(LeafInput {
            id: note.id,
            date: note.created_date()?,
            slug: slug::slugify(&note.title),
        });
    }
    let names = leaf::leaf_names(&inputs);

    let leaves = group_notes
        .iter()
        .zip(names)
        .map(|(note, name)| {
            let link = leaf_dir.join(&name);
            // The stored target is relative to the link's own directory, so the
            // vault stays relocatable (ADR 0008).
            let target = fsutil::relative_path(&leaf_dir, &note.path);
            (link, target)
        })
        .collect();
    Ok(leaves)
}

/// The view tree's current contents: every file keyed by path, plus every
/// subdirectory.
///
/// A symlink maps to `Some(target)` (its stored target, read without following);
/// any other file maps to `None`, so a stray file at a leaf path is recognized as
/// not matching its desired symlink. The directory set drives empty-directory
/// pruning. A missing view directory yields empty collections (the first build).
fn actual_state(view_dir: &Path) -> Result<(LeafMap, DirSet)> {
    let mut files = LeafMap::new();
    let mut dirs = DirSet::new();
    collect_state(view_dir, &mut files, &mut dirs)?;
    Ok((files, dirs))
}

fn collect_state(dir: &Path, files: &mut LeafMap, dirs: &mut DirSet) -> Result<()> {
    for (path, file_type) in fsutil::read_dir_entries(dir)? {
        if file_type.is_dir() {
            dirs.insert(path.clone());
            collect_state(&path, files, dirs)?;
        } else if file_type.is_symlink() {
            files.insert(path.clone(), Some(fsutil::read_link(&path)?));
        } else {
            files.insert(path, None);
        }
    }
    Ok(())
}

/// The normalized grouping values a note contributes for `field`.
///
/// `tags` is taken from the already-normalized tag list; any other field is
/// read from the raw frontmatter, accepting a scalar or a sequence and
/// normalizing each value the same way tags are (ADR 0009). Values that
/// normalize to nothing, and missing/non-scalar fields, contribute nothing, so
/// such notes are skipped.
fn group_values(note: &Note, field: &str) -> Vec<String> {
    if field == "tags" {
        return note.tags.clone();
    }

    let raw: Vec<String> = match note.frontmatter.get(Value::from(field)) {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(scalar_to_string).collect(),
        Some(scalar) => scalar_to_string(scalar).into_iter().collect(),
        None => Vec::new(),
    };

    let mut values = Vec::new();
    for entry in raw {
        let normalized = tag::normalize(&entry);
        if !normalized.is_empty() && !values.contains(&normalized) {
            values.push(normalized);
        }
    }
    values
}

/// Render a scalar YAML value to its string form, or `None` for non-scalars.
fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn note(id: &str, frontmatter: &str) -> Note {
        let content = format!("---\n{frontmatter}---\nbody\n");
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{id}-slug.md")),
            &content,
            None,
        )
        .expect("parse note")
    }

    #[test]
    fn tags_field_uses_normalized_tags() {
        let n = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "title: T\ntags: [Programming/Rust, area/work]\n",
        );
        assert_eq!(
            group_values(&n, "tags"),
            vec!["programming/rust", "area/work"]
        );
    }

    #[test]
    fn arbitrary_field_is_normalized() {
        let n = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "title: T\nstatus: In Progress\n",
        );
        assert_eq!(group_values(&n, "status"), vec!["in-progress"]);
    }

    #[test]
    fn numeric_field_groups_by_its_text() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\npriority: 2\n");
        assert_eq!(group_values(&n, "priority"), vec!["2"]);
    }

    #[test]
    fn list_field_fans_out() {
        let n = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "title: T\nareas: [Home, Work]\n",
        );
        assert_eq!(group_values(&n, "areas"), vec!["home", "work"]);
    }

    #[test]
    fn missing_field_yields_no_groups() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\n");
        assert!(group_values(&n, "status").is_empty());
    }

    #[test]
    fn unnormalizable_value_is_dropped() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\nstatus: \"!!!\"\n");
        assert!(group_values(&n, "status").is_empty());
    }
}

/// Behavioral tests for the on-disk diff in [`sync_view`], exercised against a
/// real temporary vault so every edge of the create/remove/prune logic is hit.
#[cfg(test)]
mod sync_tests {
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::scan;
    use crate::test_support::{vault_with_views, write_note};

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// Scan the vault and sync the single `by-tag` view (grouping by `tags`).
    fn sync(vault: &Vault) {
        let view = ViewDef::new("by-tag", "tags");
        let scan = scan::scan_notes_dir(&vault.layout().all_notes()).expect("scan");
        sync_view(vault, &view, &scan.notes).expect("sync view");
    }

    fn group_dir(vault: &Vault, group: &str) -> PathBuf {
        vault.root().join("by-tag").join(group)
    }

    /// The single entry inside `dir` (panics unless there is exactly one).
    fn only_entry(dir: &Path) -> PathBuf {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .collect();
        assert_eq!(entries.len(), 1, "expected exactly one entry in {dir:?}");
        entries.pop().expect("one entry")
    }

    /// The leaf in `dir` whose filename contains `needle`.
    fn leaf_containing(dir: &Path, needle: &str) -> PathBuf {
        std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .find(|p| {
                p.file_name()
                    .expect("name")
                    .to_string_lossy()
                    .contains(needle)
            })
            .unwrap_or_else(|| panic!("no leaf containing {needle:?} in {dir:?}"))
    }

    /// The inode of the symlink itself (`lstat`, not following the target).
    fn link_ino(path: &Path) -> u64 {
        std::fs::symlink_metadata(path).expect("lstat").ino()
    }

    #[test]
    fn first_sync_materializes_a_link_that_resolves_to_the_note() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );

        sync(&vault);

        let leaf = only_entry(&group_dir(&vault, "area/work"));
        assert!(leaf.exists(), "symlink resolves");
        assert!(
            std::fs::read_to_string(&leaf)
                .expect("read via link")
                .contains("title: Note")
        );
    }

    #[test]
    fn unchanged_leaf_keeps_its_inode_while_a_changed_sibling_is_relinked() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Alpha\ntags: [keep]\n---\nx\n",
        );
        let beta = write_note(
            &vault,
            &format!("{ULID_B}-b.md"),
            "---\ntitle: Beta\ntags: [keep]\n---\nx\n",
        );
        sync(&vault);

        let group = group_dir(&vault, "keep");
        let alpha = leaf_containing(&group, "alpha");
        let alpha_ino = link_ino(&alpha);
        let beta_old = leaf_containing(&group, "beta");

        // Retitle Beta out of band, then re-sync.
        std::fs::write(&beta, "---\ntitle: Renamed\ntags: [keep]\n---\nx\n").expect("rewrite");
        sync(&vault);

        // Alpha's leaf was never recreated: same path, same inode.
        assert!(alpha.exists());
        assert_eq!(
            link_ino(&alpha),
            alpha_ino,
            "an unchanged leaf must not be recreated"
        );
        // Beta's old leaf is gone; a fresh one exists under the new slug.
        assert!(!beta_old.exists());
        assert!(leaf_containing(&group, "renamed").exists());
    }

    #[test]
    fn retitling_a_note_replaces_its_leaf() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        let path = write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Old Title\ntags: [t]\n---\nx\n",
        );
        sync(&vault);
        let old = only_entry(&group_dir(&vault, "t"));
        assert!(
            old.file_name()
                .expect("name")
                .to_string_lossy()
                .contains("old-title")
        );

        std::fs::write(&path, "---\ntitle: New Title\ntags: [t]\n---\nx\n").expect("rewrite");
        sync(&vault);

        assert!(!old.exists(), "old leaf removed");
        let new = only_entry(&group_dir(&vault, "t"));
        assert!(
            new.file_name()
                .expect("name")
                .to_string_lossy()
                .contains("new-title")
        );
    }

    #[test]
    fn a_leaf_with_a_drifted_target_is_relinked() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [t]\n---\nx\n",
        );
        sync(&vault);
        let leaf = only_entry(&group_dir(&vault, "t"));

        // Point the leaf at the wrong target, out of band.
        std::fs::remove_file(&leaf).expect("rm");
        std::os::unix::fs::symlink("../../all-notes/bogus.md", &leaf).expect("bad symlink");

        sync(&vault);

        let target = std::fs::read_link(&leaf).expect("readlink");
        assert!(
            target.to_string_lossy().ends_with(&format!("{ULID_A}-n.md")),
            "target corrected, got {target:?}"
        );
        assert!(leaf.exists(), "resolves again");
    }

    #[test]
    fn deleting_a_note_prunes_its_leaf_and_the_emptied_group() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        let path = write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [solo]\n---\nx\n",
        );
        sync(&vault);
        assert!(group_dir(&vault, "solo").is_dir());

        std::fs::remove_file(&path).expect("rm note");
        sync(&vault);

        assert!(!group_dir(&vault, "solo").exists(), "emptied group pruned");
        assert!(vault.root().join("by-tag").is_dir(), "view root kept");
    }

    #[test]
    fn emptying_a_nested_group_prunes_the_chain_but_keeps_the_view_root() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        let path = write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [area/work/deep]\n---\nx\n",
        );
        sync(&vault);
        assert!(vault.root().join("by-tag/area/work/deep").is_dir());

        std::fs::remove_file(&path).expect("rm");
        sync(&vault);

        assert!(
            !vault.root().join("by-tag/area").exists(),
            "the whole empty chain is pruned"
        );
        assert!(
            vault.root().join("by-tag").is_dir(),
            "the view root is kept even when empty"
        );
    }

    #[test]
    fn a_pre_existing_stray_empty_subdir_is_pruned() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [real]\n---\nx\n",
        );
        sync(&vault);
        // A stray empty group directory left out of band.
        std::fs::create_dir_all(vault.root().join("by-tag/ghost")).expect("mkdir ghost");

        sync(&vault);

        assert!(
            !vault.root().join("by-tag/ghost").exists(),
            "stray empty directory pruned"
        );
        assert!(group_dir(&vault, "real").is_dir(), "real group kept");
    }

    #[test]
    fn a_stray_file_at_a_leaf_path_is_replaced_with_the_symlink() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [t]\n---\nx\n",
        );
        sync(&vault);
        let leaf = only_entry(&group_dir(&vault, "t"));

        // Replace the symlink with a regular file at the same path.
        std::fs::remove_file(&leaf).expect("rm");
        std::fs::write(&leaf, b"not a symlink").expect("write file");

        sync(&vault);

        assert!(
            std::fs::symlink_metadata(&leaf)
                .expect("lstat")
                .file_type()
                .is_symlink(),
            "regular file replaced by a symlink"
        );
        assert!(
            std::fs::read_to_string(&leaf)
                .expect("read")
                .contains("title: Note")
        );
    }

    #[test]
    fn a_stray_non_leaf_file_inside_a_group_is_removed() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [t]\n---\nx\n",
        );
        sync(&vault);
        let stray = group_dir(&vault, "t").join("README.txt");
        std::fs::write(&stray, b"junk").expect("write stray");

        sync(&vault);

        assert!(!stray.exists(), "stray non-leaf file removed");
        assert_eq!(
            std::fs::read_dir(group_dir(&vault, "t")).expect("read").count(),
            1,
            "the real leaf survives"
        );
    }

    #[test]
    fn list_valued_tags_place_the_note_under_each_value() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [home, work]\n---\nx\n",
        );
        sync(&vault);
        assert!(only_entry(&group_dir(&vault, "home")).exists());
        assert!(only_entry(&group_dir(&vault, "work")).exists());
    }

    #[test]
    fn collision_disambiguates_then_reshuffles_when_a_collider_is_removed() {
        // Two ULIDs sharing a timestamp prefix produce the same date, so equal
        // titles collide on `<date>-<slug>` and each gains a ULID tail.
        const C1: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        const C2: &str = "01ARZ3NDEKTSV4RRFFQ69G5FBW";
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &vault,
            &format!("{C1}-a.md"),
            "---\ntitle: Review\ntags: [dup]\n---\nx\n",
        );
        let second = write_note(
            &vault,
            &format!("{C2}-b.md"),
            "---\ntitle: Review\ntags: [dup]\n---\nx\n",
        );
        sync(&vault);

        let group = group_dir(&vault, "dup");
        assert_eq!(std::fs::read_dir(&group).expect("read").count(), 2);
        assert!(leaf_containing(&group, "review-FAV").exists());
        assert!(leaf_containing(&group, "review-FBW").exists());

        // Remove one collider: the survivor reverts to the undisambiguated name.
        std::fs::remove_file(&second).expect("rm");
        sync(&vault);
        let leaf = only_entry(&group);
        let name = leaf.file_name().expect("name").to_string_lossy().into_owned();
        assert!(name.ends_with("review.md"), "expected reshuffle, got {name}");
    }

    /// Recursively collect every symlink under `root` as `(path relative to
    /// `strip`, stored target)`, sorted. Stripping the per-tempdir prefix makes
    /// trees from two independent vaults directly comparable; targets are already
    /// stored relative to their link, so they need no stripping.
    fn collect_tree(root: &Path, strip: &Path) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read dir") {
                let path = entry.expect("entry").path();
                let file_type = std::fs::symlink_metadata(&path).expect("lstat").file_type();
                if file_type.is_symlink() {
                    let rel = path.strip_prefix(strip).expect("strip prefix");
                    let target = std::fs::read_link(&path).expect("readlink");
                    out.push((
                        rel.to_string_lossy().into_owned(),
                        target.to_string_lossy().into_owned(),
                    ));
                } else if file_type.is_dir() {
                    stack.push(path);
                }
            }
        }
        out.sort();
        out
    }

    /// A larger corpus across many tag groups, nested paths, and several
    /// intra-group title collisions, materialized into two fresh vaults. The two
    /// trees must be byte-identical (parallel grouping/merge stays deterministic)
    /// and must equal the expected sorted leaf set.
    #[test]
    fn parallel_desired_links_is_deterministic_across_two_vaults() {
        // Distinct ULIDs; the first three share a timestamp prefix so equal
        // titles in the same group collide and gain a ULID tail.
        const IDS: &[&str] = &[
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "01ARZ3NDEKTSV4RRFFQ69G5FBW",
            "01ARZ3NDEKTSV4RRFFQ69G5FCX",
            "01BX5ZZKBKACTAV9WEVGEMMVRZ",
            "01C0AAAAAAAAAAAAAAAAAAAAAA",
            "01D0BBBBBBBBBBBBBBBBBBBBBB",
            "01E0CCCCCCCCCCCCCCCCCCCCCC",
            "01F0DDDDDDDDDDDDDDDDDDDDDD",
        ];

        // (filename suffix, title, tags) — many groups, nested paths, and the
        // first three colliding on "Review" within `dup`.
        let corpus: &[(&str, &str, &str)] = &[
            ("a.md", "Review", "[dup, area/work]"),
            ("b.md", "Review", "[dup, area/work/deep]"),
            ("c.md", "Review", "[dup]"),
            ("d.md", "Planning", "[area/home, status/active]"),
            ("e.md", "Notes", "[status/active]"),
            ("f.md", "Ideas", "[area/home, area/work]"),
            ("g.md", "Log", "[journal/2026, journal/2026/06]"),
            ("h.md", "Recipe", "[cooking]"),
        ];

        let build = || {
            let (guard, vault) = vault_with_views(&[("by-tag", "tags")]);
            for (id, (suffix, title, tags)) in IDS.iter().zip(corpus) {
                write_note(
                    &vault,
                    &format!("{id}-{suffix}"),
                    &format!("---\ntitle: {title}\ntags: {tags}\n---\nx\n"),
                );
            }
            sync(&vault);
            let tree = collect_tree(&vault.root().join("by-tag"), vault.root());
            (guard, tree)
        };

        let (_g1, tree1) = build();
        let (_g2, tree2) = build();

        assert_eq!(tree1, tree2, "two independent vaults must materialize identically");

        // The leaf paths the corpus must produce, independent of disambiguation
        // tails which are asserted structurally below.
        let leaf_paths: Vec<&str> = tree1.iter().map(|(p, _)| p.as_str()).collect();
        // Leaf placements summed over the corpus's per-note tag fan-out
        // (2+2+1+2+1+2+2+1).
        assert_eq!(leaf_paths.len(), 13, "expected 13 leaf placements, got {leaf_paths:?}");
        // The three colliding "Review" notes each land in `dup` with a ULID tail.
        let dup_leaves: Vec<&&str> = leaf_paths
            .iter()
            .filter(|p| p.starts_with("by-tag/dup/"))
            .collect();
        assert_eq!(dup_leaves.len(), 3, "all three colliders placed in dup");
        for tail in ["FAV", "FBW", "FCX"] {
            assert!(
                dup_leaves.iter().any(|p| p.contains(tail)),
                "collider {tail} disambiguated in dup, got {dup_leaves:?}"
            );
        }
        // Every stored target resolves (relative target back into all-notes).
        for (_, target) in &tree1 {
            assert!(target.contains("all-notes/"), "target points into all-notes: {target}");
        }
    }

    #[test]
    fn a_view_with_no_matching_notes_keeps_an_empty_root() {
        let (_g, vault) = vault_with_views(&[("by-tag", "tags")]);
        // A note with no tags contributes nothing to the view.
        write_note(
            &vault,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\n---\nx\n",
        );
        sync(&vault);

        let root = vault.root().join("by-tag");
        assert!(root.is_dir(), "view root exists");
        assert_eq!(
            std::fs::read_dir(&root).expect("read").count(),
            0,
            "but is empty"
        );
    }
}
