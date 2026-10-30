// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Materializing one view as a symlink tree (ADRs 0008, 0009, 0023, 0077).
//!
//! A view groups notes by one frontmatter field. Each grouping value (always
//! normalized) becomes a directory path under the view: the slug of each of
//! its `/`-separated segments, so `milestone/0.13` is `milestone/0-13`, and a
//! list-valued field places a note under each of its values. Values whose
//! paths are equal share one directory. The leaf in each group is a relative
//! symlink back into `all-notes/`, named `<date>-<slug>.md` with collisions
//! disambiguated (see [`super::leaf`]). A note with no value for the field, or
//! only values that have no path segment, is skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
    let view_dir = super::contained_dir(vault.layout(), &view.name)?;

    let desired = desired_links(&view_dir, view, notes)?;
    let (actual, dirs) = actual_state(&view_dir)?;

    // A configured view always has a root directory, even with no matching notes.
    fsutil::create_dir_all(&view_dir)?;

    // Remove every on-disk entry that is not already a correct leaf: a stale
    // leaf, a leaf whose target drifted (removed here, recreated just below), and
    // any non-symlink file (`target` is `None`), which is never a correct leaf and
    // so is always removed — whether it sits at a desired path or a stray one.
    for (path, target) in &actual {
        let is_correct_leaf =
            matches!(target, Some(existing) if desired.get(path) == Some(existing));
        if !is_correct_leaf {
            fsutil::remove_file(path)?;
        }
    }

    // Create the links that are missing, or were just removed for a retarget.
    // `symlink` creates any missing parent directories.
    for (path, target) in &desired {
        let already_correct =
            matches!(actual.get(path), Some(Some(existing)) if existing == target);
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
    // Group notes by the directory their value maps to. Values that differ only
    // in punctuation (`node.js`, `node-js`) share a directory and are merged
    // into one group, and a value with no usable path segment (`...`) has no
    // directory at all. A `BTreeMap` keeps the projection deterministic, which
    // matters for reproducible disambiguation and tests.
    let mut groups: BTreeMap<String, Vec<&Note>> = BTreeMap::new();
    for note in notes {
        for value in group_values(note, &view.field) {
            let path = tag::path(&value);
            if path.is_empty() {
                continue;
            }
            let members = groups.entry(path).or_default();
            // A note carrying two values on one path still gets a single leaf.
            if !members.iter().any(|member| member.id == note.id) {
                members.push(note);
            }
        }
    }

    let mut desired = BTreeMap::new();
    for (path, group_notes) in groups {
        // The path's `/` segments nest into subdirectories.
        let leaf_dir = view_dir.join(&path);

        let mut inputs = Vec::with_capacity(group_notes.len());
        for note in &group_notes {
            inputs.push(LeafInput {
                id: note.id,
                date: note.created_date()?,
                slug: slug::slugify(&note.title),
            });
        }
        let names = leaf::leaf_names(&inputs);

        for (note, name) in group_notes.iter().zip(names) {
            let link = leaf_dir.join(&name);
            // The stored target is relative to the link's own directory, so the
            // vault stays relocatable (ADR 0008).
            let target = fsutil::relative_path(&leaf_dir, &note.path);
            desired.insert(link, target);
        }
    }
    Ok(desired)
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
pub fn group_values(note: &Note, field: &str) -> Vec<String> {
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
    fn a_value_without_any_segment_is_dropped() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\nstatus: \"///\"\n");
        assert!(group_values(&n, "status").is_empty());
    }

    #[test]
    fn punctuation_in_a_field_value_is_kept() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\nversion: \"1.5\"\n");
        assert_eq!(group_values(&n, "version"), vec!["1.5"]);
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
    use crate::session::VaultSession;
    use crate::test_support::{vault_with_views, write_note};

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// Scan the vault and sync the single `by-tag` view (grouping by `tags`).
    fn sync(session: &VaultSession) {
        let view = ViewDef::parse("by-tag", "tags").expect("valid view name");
        let scan = scan::scan_notes_dir(
            &session.layout().all_notes(),
            &crate::cipher::PlaintextCipher,
        )
        .expect("scan");
        sync_view(session, &view, &scan.notes).expect("sync view");
    }

    fn group_dir(session: &VaultSession, group: &str) -> PathBuf {
        session.root().join("by-tag").join(group)
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
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );

        sync(&session);

        let leaf = only_entry(&group_dir(&session, "area/work"));
        assert!(leaf.exists(), "symlink resolves");
        assert!(
            std::fs::read_to_string(&leaf)
                .expect("read via link")
                .contains("title: Note")
        );
    }

    #[test]
    fn unchanged_leaf_keeps_its_inode_while_a_changed_sibling_is_relinked() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Alpha\ntags: [keep]\n---\nx\n",
        );
        let beta = write_note(
            &session,
            &format!("{ULID_B}-b.md"),
            "---\ntitle: Beta\ntags: [keep]\n---\nx\n",
        );
        sync(&session);

        let group = group_dir(&session, "keep");
        let alpha = leaf_containing(&group, "alpha");
        let alpha_ino = link_ino(&alpha);
        let beta_old = leaf_containing(&group, "beta");

        // Retitle Beta out of band, then re-sync.
        std::fs::write(&beta, "---\ntitle: Renamed\ntags: [keep]\n---\nx\n").expect("rewrite");
        sync(&session);

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
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        let path = write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Old Title\ntags: [t]\n---\nx\n",
        );
        sync(&session);
        let old = only_entry(&group_dir(&session, "t"));
        assert!(
            old.file_name()
                .expect("name")
                .to_string_lossy()
                .contains("old-title")
        );

        std::fs::write(&path, "---\ntitle: New Title\ntags: [t]\n---\nx\n").expect("rewrite");
        sync(&session);

        assert!(!old.exists(), "old leaf removed");
        let new = only_entry(&group_dir(&session, "t"));
        assert!(
            new.file_name()
                .expect("name")
                .to_string_lossy()
                .contains("new-title")
        );
    }

    #[test]
    fn a_leaf_with_a_drifted_target_is_relinked() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [t]\n---\nx\n",
        );
        sync(&session);
        let leaf = only_entry(&group_dir(&session, "t"));

        // Point the leaf at the wrong target, out of band.
        std::fs::remove_file(&leaf).expect("rm");
        std::os::unix::fs::symlink("../../all-notes/bogus.md", &leaf).expect("bad symlink");

        sync(&session);

        let target = std::fs::read_link(&leaf).expect("readlink");
        assert!(
            target
                .to_string_lossy()
                .ends_with(&format!("{ULID_A}-n.md")),
            "target corrected, got {target:?}"
        );
        assert!(leaf.exists(), "resolves again");
    }

    #[test]
    fn deleting_a_note_prunes_its_leaf_and_the_emptied_group() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        let path = write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [solo]\n---\nx\n",
        );
        sync(&session);
        assert!(group_dir(&session, "solo").is_dir());

        std::fs::remove_file(&path).expect("rm note");
        sync(&session);

        assert!(
            !group_dir(&session, "solo").exists(),
            "emptied group pruned"
        );
        assert!(session.root().join("by-tag").is_dir(), "view root kept");
    }

    #[test]
    fn emptying_a_nested_group_prunes_the_chain_but_keeps_the_view_root() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        let path = write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [area/work/deep]\n---\nx\n",
        );
        sync(&session);
        assert!(session.root().join("by-tag/area/work/deep").is_dir());

        std::fs::remove_file(&path).expect("rm");
        sync(&session);

        assert!(
            !session.root().join("by-tag/area").exists(),
            "the whole empty chain is pruned"
        );
        assert!(
            session.root().join("by-tag").is_dir(),
            "the view root is kept even when empty"
        );
    }

    #[test]
    fn a_pre_existing_stray_empty_subdir_is_pruned() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [real]\n---\nx\n",
        );
        sync(&session);
        // A stray empty group directory left out of band.
        std::fs::create_dir_all(session.root().join("by-tag/ghost")).expect("mkdir ghost");

        sync(&session);

        assert!(
            !session.root().join("by-tag/ghost").exists(),
            "stray empty directory pruned"
        );
        assert!(group_dir(&session, "real").is_dir(), "real group kept");
    }

    #[test]
    fn a_stray_file_at_a_leaf_path_is_replaced_with_the_symlink() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [t]\n---\nx\n",
        );
        sync(&session);
        let leaf = only_entry(&group_dir(&session, "t"));

        // Replace the symlink with a regular file at the same path.
        std::fs::remove_file(&leaf).expect("rm");
        std::fs::write(&leaf, b"not a symlink").expect("write file");

        sync(&session);

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
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [t]\n---\nx\n",
        );
        sync(&session);
        let stray = group_dir(&session, "t").join("README.txt");
        std::fs::write(&stray, b"junk").expect("write stray");

        sync(&session);

        assert!(!stray.exists(), "stray non-leaf file removed");
        assert_eq!(
            std::fs::read_dir(group_dir(&session, "t"))
                .expect("read")
                .count(),
            1,
            "the real leaf survives"
        );
    }

    #[test]
    fn list_valued_tags_place_the_note_under_each_value() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [home, work]\n---\nx\n",
        );
        sync(&session);
        assert!(only_entry(&group_dir(&session, "home")).exists());
        assert!(only_entry(&group_dir(&session, "work")).exists());
    }

    #[test]
    fn collision_disambiguates_then_reshuffles_when_a_collider_is_removed() {
        // Two ULIDs sharing a timestamp prefix produce the same date, so equal
        // titles collide on `<date>-<slug>` and each gains a ULID tail.
        const C1: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        const C2: &str = "01ARZ3NDEKTSV4RRFFQ69G5FBW";
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{C1}-a.md"),
            "---\ntitle: Review\ntags: [dup]\n---\nx\n",
        );
        let second = write_note(
            &session,
            &format!("{C2}-b.md"),
            "---\ntitle: Review\ntags: [dup]\n---\nx\n",
        );
        sync(&session);

        let group = group_dir(&session, "dup");
        assert_eq!(std::fs::read_dir(&group).expect("read").count(), 2);
        assert!(leaf_containing(&group, "review-FAV").exists());
        assert!(leaf_containing(&group, "review-FBW").exists());

        // Remove one collider: the survivor reverts to the undisambiguated name.
        std::fs::remove_file(&second).expect("rm");
        sync(&session);
        let leaf = only_entry(&group);
        let name = leaf
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        assert!(
            name.ends_with("review.md"),
            "expected reshuffle, got {name}"
        );
    }

    #[test]
    fn a_view_with_no_matching_notes_keeps_an_empty_root() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        // A note with no tags contributes nothing to the view.
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\n---\nx\n",
        );
        sync(&session);

        let root = session.root().join("by-tag");
        assert!(root.is_dir(), "view root exists");
        assert_eq!(
            std::fs::read_dir(&root).expect("read").count(),
            0,
            "but is empty"
        );
    }

    /// Two notes titled `Review` on one day, and a third on that day whose
    /// title slugifies to `review-<suffix>`, all tagged `t`. The ULIDs end in
    /// `tail_a` and `tail_b`, so `Review`'s disambiguated names are
    /// `…-review-<tail_a>` and `…-review-<tail_b>`.
    fn colliding_reviews(session: &VaultSession, tail_a: &str, suffix: &str) {
        let prefix = "01ARZ3NDEKTSV4RRFFQ69G5";
        write_note(
            session,
            &format!("{prefix}{tail_a}-review.md"),
            "---\ntitle: Review\ntags: [t]\n---\nx\n",
        );
        write_note(
            session,
            &format!("{prefix}FBW-review.md"),
            "---\ntitle: Review\ntags: [t]\n---\nx\n",
        );
        write_note(
            session,
            &format!("{prefix}YYY-review-{suffix}.md"),
            &format!("---\ntitle: Review {suffix}\ntags: [t]\n---\nx\n"),
        );
    }

    #[test]
    fn a_note_whose_name_equals_a_disambiguated_name_keeps_its_leaf() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        colliding_reviews(&session, "123", "123");

        sync(&session);

        assert_eq!(
            std::fs::read_dir(group_dir(&session, "t"))
                .expect("read")
                .count(),
            3,
            "every note has a leaf"
        );
    }

    #[test]
    fn leaves_differing_only_in_case_both_exist() {
        // On a case-insensitive filesystem `…-review-FAV.md` and
        // `…-review-fav.md` are one entry, so the second link could not be
        // made; on a case-sensitive one the names already differ.
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        colliding_reviews(&session, "FAV", "fav");

        sync(&session);

        assert_eq!(
            std::fs::read_dir(group_dir(&session, "t"))
                .expect("read")
                .count(),
            3,
            "every note has a leaf"
        );
    }

    /// The directory names under the `by-tag` view, as paths relative to it.
    fn group_dirs(session: &VaultSession) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).expect("read dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    out.push(
                        path.strip_prefix(base)
                            .expect("below the view")
                            .to_string_lossy()
                            .into_owned(),
                    );
                    walk(&path, base, out);
                }
            }
        }
        let base = session.root().join("by-tag");
        let mut out = Vec::new();
        walk(&base, &base, &mut out);
        out.sort();
        out
    }

    #[test]
    fn a_dotted_tag_gets_a_dashed_directory() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [milestone/0.13]\n---\nx\n",
        );

        sync(&session);

        assert_eq!(group_dirs(&session), ["milestone", "milestone/0-13"]);
    }

    #[test]
    fn a_tag_cannot_climb_out_of_the_view() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [\"../escape\"]\n---\nx\n",
        );

        sync(&session);

        assert_eq!(group_dirs(&session), ["escape"]);
        assert!(!session.root().join("escape").exists());
    }

    #[test]
    fn a_tag_of_only_punctuation_gets_no_directory() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-n.md"),
            "---\ntitle: Note\ntags: [\"...\"]\n---\nx\n",
        );

        sync(&session);

        assert!(group_dirs(&session).is_empty());
    }

    #[test]
    fn tags_sharing_a_path_share_one_directory() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Alpha\ntags: [node.js]\n---\nx\n",
        );
        write_note(
            &session,
            &format!("{ULID_B}-b.md"),
            "---\ntitle: Beta\ntags: [node-js]\n---\nx\n",
        );

        sync(&session);

        assert_eq!(group_dirs(&session), ["node-js"]);
        let group = group_dir(&session, "node-js");
        leaf_containing(&group, "alpha");
        leaf_containing(&group, "beta");
    }

    #[test]
    fn a_note_with_two_tags_on_one_path_has_one_leaf() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Alpha\ntags: [node.js, node-js]\n---\nx\n",
        );

        sync(&session);

        only_entry(&group_dir(&session, "node-js"));
    }

    #[test]
    fn notes_with_equal_names_in_tags_sharing_a_path_get_ulid_tails() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Same\ntags: [node.js]\n---\nx\n",
        );
        write_note(
            &session,
            &format!("{ULID_B}-b.md"),
            "---\ntitle: Same\ntags: [node-js]\n---\nx\n",
        );

        sync(&session);

        let entries = std::fs::read_dir(group_dir(&session, "node-js"))
            .expect("read dir")
            .count();
        assert_eq!(entries, 2, "both notes keep a leaf");
    }

    #[test]
    fn the_directory_of_an_older_dropping_rule_is_replaced() {
        let (_g, session) = vault_with_views(&[("by-tag", "tags")]);
        write_note(
            &session,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Alpha\ntags: [milestone/0.13]\n---\nx\n",
        );
        let stale = session.root().join("by-tag/milestone/013");
        std::fs::create_dir_all(&stale).expect("stale group");
        std::os::unix::fs::symlink(
            format!("../../../all-notes/{ULID_A}-a.md"),
            stale.join("2016-07-31-alpha.md"),
        )
        .expect("stale link");

        sync(&session);

        assert_eq!(group_dirs(&session), ["milestone", "milestone/0-13"]);
    }

    #[test]
    fn a_field_value_with_a_dot_groups_under_a_dashed_directory() {
        let (_g, session) = vault_with_views(&[("by-version", "version")]);
        write_note(
            &session,
            &format!("{ULID_A}-a.md"),
            "---\ntitle: Alpha\nversion: \"1.5\"\n---\nx\n",
        );
        let view = ViewDef::parse("by-version", "version").expect("valid view name");
        let scan = scan::scan_notes_dir(
            &session.layout().all_notes(),
            &crate::cipher::PlaintextCipher,
        )
        .expect("scan");

        sync_view(&session, &view, &scan.notes).expect("sync view");

        assert!(session.root().join("by-version/1-5").is_dir());
    }
}
