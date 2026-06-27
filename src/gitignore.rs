// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Maintaining the vault's root `.gitignore` for materialized views (ADR 0032).
//!
//! The materialized view directories are derived symlink trees, regenerated
//! from frontmatter on demand, so they do not belong in version control. This
//! module keeps a single root `.gitignore` whose ntropy-managed entries mirror
//! exactly the configured views, while leaving any line the user wrote
//! untouched.
//!
//! Ownership is tracked by a marker comment placed directly above each managed
//! entry. An entry is ntropy's to prune only when that exact comment sits on
//! the line above it; anything else is the user's and is never removed, even
//! when it happens to look like a view entry. Conversely, an entry is
//! considered *present* (and so not re-added) whenever any line — the user's or
//! ours — names the same directory, so we never duplicate an ignore the user
//! added by hand.
//!
//! The module is decoupled from the config layer: callers pass the configured
//! view names, exactly as the `view` layer takes `ViewDef`s. The on-disk entry
//! is derived from [`Layout::view_dir`] so this module owns only git syntax,
//! never *where* a view lives.

use crate::error::Result;
use crate::fsutil;
use crate::vault::Vault;

/// The comment written directly above every ntropy-managed entry.
///
/// It explains the entry to a reader and, just as importantly, marks the entry
/// as ntropy-owned: only an entry carrying this exact line above it is pruned
/// when its view leaves the configuration.
pub const MARKER: &str = "# ntropy: derived view directory, safe to ignore";

/// The outcome of reconciling `.gitignore` content against the configured views.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    /// The rewritten file content, or `None` when nothing needed to change.
    pub content: Option<String>,
    /// Entries newly added, in the order they were appended.
    pub added: Vec<String>,
    /// Managed entries pruned because their view is no longer configured.
    pub removed: Vec<String>,
}

/// Reconcile existing `.gitignore` content so its managed entries match exactly
/// the `configured` view entries (each in anchored `/<rel>/` form).
///
/// Two passes operate on the file's lines. The prune pass drops every
/// ntropy-owned entry whose view is no longer configured, taking its marker
/// comment with it. The add pass appends a `marker + entry` block for every
/// configured entry not already present in some form. User-authored lines are
/// never removed or reordered.
pub fn sync_entries(existing: &str, configured: &[&str]) -> SyncOutcome {
    let configured_norm: Vec<String> = configured.iter().map(|e| normalize(e)).collect();
    let logical = logical_lines(existing);

    // Prune pass: drop managed entries whose view has gone, and the marker
    // comment that sits directly above each of them.
    let mut kept: Vec<&str> = Vec::with_capacity(logical.len());
    let mut removed: Vec<String> = Vec::new();
    for (idx, line) in logical.iter().enumerate() {
        let owned = idx > 0 && logical[idx - 1].trim() == MARKER && is_entry(line);
        if owned {
            let name = normalize(line);
            if !configured_norm.contains(&name) {
                // The previous line was this entry's marker; it was kept on the
                // prior step, so removing it here drops the whole block.
                kept.pop();
                removed.push(format!("/{name}/"));
                continue;
            }
        }
        kept.push(line);
    }

    // Add pass: an entry is "present" if any surviving entry line names the same
    // directory, regardless of who wrote it or in which slash form.
    let mut present: Vec<String> = kept
        .iter()
        .filter(|l| is_entry(l))
        .map(|l| normalize(l))
        .collect();
    let mut added: Vec<String> = Vec::new();
    let mut blocks: Vec<&str> = Vec::new();
    for (entry, name) in configured.iter().copied().zip(configured_norm.iter()) {
        if present.iter().any(|p| p == name) {
            continue;
        }
        present.push(name.clone());
        added.push(entry.to_string());
        blocks.push(MARKER);
        blocks.push(entry);
    }

    // Leaving the file byte-for-byte untouched when there is nothing to do keeps
    // repeated runs idempotent and never rewrites a user's trailing-newline style.
    if added.is_empty() && removed.is_empty() {
        return SyncOutcome {
            content: None,
            added,
            removed,
        };
    }

    let mut lines = kept;
    lines.extend(blocks);
    let content = if lines.is_empty() {
        // Every entry was pruned: an empty file remains (we never delete it).
        String::new()
    } else {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    };
    SyncOutcome {
        content: Some(content),
        added,
        removed,
    }
}

/// What [`sync`] changed, for human-facing reporting.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Entries newly added (in anchored `/<rel>/` form).
    pub added: Vec<String>,
    /// Managed entries pruned because their view is no longer configured.
    pub removed: Vec<String>,
}

/// Reconcile `<vault>/.gitignore` so its managed entries match the configured
/// views, writing the file only when something changed.
///
/// Each view's ignore entry is derived from [`Layout::view_dir`] relative to the
/// vault root, then wrapped in anchored git syntax (`/<rel>/`). A missing file
/// is treated as empty; the write goes through [`fsutil::atomic_write`].
///
/// [`Layout::view_dir`]: crate::vault::layout::Layout::view_dir
pub fn sync(vault: &Vault, configured_view_names: &[&str]) -> Result<SyncReport> {
    let layout = vault.layout();
    let root = layout.root();
    let entries: Vec<String> = configured_view_names
        .iter()
        .map(|name| {
            let dir = layout.view_dir(name);
            let rel = dir
                .strip_prefix(root)
                .expect("view dir is built by joining the name onto the root");
            format!("/{}/", rel.display())
        })
        .collect();
    let entry_refs: Vec<&str> = entries.iter().map(String::as_str).collect();

    let path = layout.gitignore_file();
    let existing = fsutil::read_to_string_if_exists(&path)?.unwrap_or_default();
    let outcome = sync_entries(&existing, &entry_refs);
    if let Some(content) = outcome.content {
        fsutil::atomic_write(&path, content.as_bytes())?;
    }
    Ok(SyncReport {
        added: outcome.added,
        removed: outcome.removed,
    })
}

/// The file's lines, with the trailing empty element produced by a final
/// newline dropped so it is not mistaken for a blank content line.
///
/// `split('\n')` is information-preserving: every line keeps its exact bytes,
/// and a file ending in `\n` yields a trailing `""` which this strips.
fn logical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<&str> = text.split('\n').collect();
    if matches!(parts.last(), Some(&"")) {
        parts.pop();
    }
    parts
}

/// The directory a line names, ignoring leading/trailing slashes and surrounding
/// whitespace, so `/by-tag/`, `by-tag/`, `/by-tag` and `by-tag` all compare equal.
fn normalize(line: &str) -> String {
    line.trim()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .trim()
        .to_string()
}

/// Whether a line is an ignore entry rather than a comment or blank line.
fn is_entry(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && !trimmed.starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_entry_to_empty_file() {
        let out = sync_entries("", &["/by-tag/"]);
        assert_eq!(out.added, ["/by-tag/"]);
        assert!(out.removed.is_empty());
        insta::assert_snapshot!(out.content.unwrap(), @r"
        # ntropy: derived view directory, safe to ignore
        /by-tag/
        ");
    }

    #[test]
    fn appends_after_user_content_without_trailing_newline() {
        // No trailing newline on the user's line: the block still lands cleanly.
        let out = sync_entries("*.tmp", &["/by-tag/"]);
        assert_eq!(out.added, ["/by-tag/"]);
        insta::assert_snapshot!(out.content.unwrap(), @r"
        *.tmp
        # ntropy: derived view directory, safe to ignore
        /by-tag/
        ");
    }

    #[test]
    fn idempotent_when_block_already_present() {
        let existing = "# ntropy: derived view directory, safe to ignore\n/by-tag/\n";
        let out = sync_entries(existing, &["/by-tag/"]);
        assert_eq!(out.content, None);
        assert!(out.added.is_empty());
        assert!(out.removed.is_empty());
    }

    #[test]
    fn does_not_duplicate_a_user_authored_entry() {
        // The user added the ignore themselves, in bare form and without a marker.
        let out = sync_entries("by-tag\n", &["/by-tag/"]);
        assert_eq!(out.content, None);
        assert!(out.added.is_empty());
    }

    #[test]
    fn membership_tolerates_slash_variants() {
        for form in ["by-tag", "by-tag/", "/by-tag", "/by-tag/", "  /by-tag/  "] {
            let out = sync_entries(&format!("{form}\n"), &["/by-tag/"]);
            assert_eq!(out.content, None, "form `{form}` should count as present");
        }
    }

    #[test]
    fn prunes_managed_orphan_with_its_comment() {
        let existing = "\
*.tmp
# ntropy: derived view directory, safe to ignore
/old/
";
        let out = sync_entries(existing, &[]);
        assert_eq!(out.removed, ["/old/"]);
        assert!(out.added.is_empty());
        insta::assert_snapshot!(out.content.unwrap(), @"*.tmp");
    }

    #[test]
    fn keeps_user_lookalike_without_marker() {
        // Same directory, but no marker above it: it is the user's, so we leave it.
        let out = sync_entries("/old/\n", &[]);
        assert_eq!(out.content, None);
        assert!(out.removed.is_empty());
    }

    #[test]
    fn adds_and_prunes_in_one_call() {
        let existing = "# ntropy: derived view directory, safe to ignore\n/old/\n";
        let out = sync_entries(existing, &["/by-tag/"]);
        assert_eq!(out.added, ["/by-tag/"]);
        assert_eq!(out.removed, ["/old/"]);
        insta::assert_snapshot!(out.content.unwrap(), @r"
        # ntropy: derived view directory, safe to ignore
        /by-tag/
        ");
    }

    #[test]
    fn preserves_unrelated_user_lines_and_order() {
        let existing = "\
# my own notes
build/
*.log

# ntropy: derived view directory, safe to ignore
/old/
secrets/
";
        let out = sync_entries(existing, &["/by-tag/"]);
        assert_eq!(out.added, ["/by-tag/"]);
        assert_eq!(out.removed, ["/old/"]);
        insta::assert_snapshot!(out.content.unwrap(), @r"
        # my own notes
        build/
        *.log

        secrets/
        # ntropy: derived view directory, safe to ignore
        /by-tag/
        ");
    }

    #[test]
    fn adds_only_the_missing_view_in_partial_overlap() {
        let existing = "# ntropy: derived view directory, safe to ignore\n/by-tag/\n";
        let out = sync_entries(existing, &["/by-tag/", "/by-status/"]);
        assert_eq!(out.added, ["/by-status/"]);
        assert!(out.removed.is_empty());
        insta::assert_snapshot!(out.content.unwrap(), @r"
        # ntropy: derived view directory, safe to ignore
        /by-tag/
        # ntropy: derived view directory, safe to ignore
        /by-status/
        ");
    }

    #[test]
    fn pruning_every_view_leaves_an_empty_file() {
        let existing = "\
# ntropy: derived view directory, safe to ignore
/by-tag/
# ntropy: derived view directory, safe to ignore
/by-status/
";
        let out = sync_entries(existing, &[]);
        assert_eq!(out.removed, ["/by-tag/", "/by-status/"]);
        assert_eq!(out.content.as_deref(), Some(""));
    }

    #[test]
    fn handles_multi_segment_view_names() {
        let added = sync_entries("", &["/area/work/"]);
        assert_eq!(added.added, ["/area/work/"]);

        let existing = "# ntropy: derived view directory, safe to ignore\n/area/work/\n";
        let pruned = sync_entries(existing, &[]);
        assert_eq!(pruned.removed, ["/area/work/"]);
        assert_eq!(pruned.content.as_deref(), Some(""));
    }

    // -- IO shell ------------------------------------------------------------

    #[test]
    fn sync_creates_file_with_anchored_entry() {
        let dir = tempfile::tempdir().expect("temp dir");
        let v = Vault::new(dir.path());
        let report = sync(&v, &["by-tag"]).expect("sync");
        assert_eq!(report.added, ["/by-tag/"]);
        let content = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        assert!(content.contains("/by-tag/"), "content: {content}");
        assert!(content.contains(MARKER), "content: {content}");
    }

    #[test]
    fn sync_is_idempotent_on_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let v = Vault::new(dir.path());
        sync(&v, &["by-tag"]).expect("first");
        let before = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        let report = sync(&v, &["by-tag"]).expect("second");
        assert!(report.added.is_empty() && report.removed.is_empty());
        let after = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        assert_eq!(before, after);
    }

    #[test]
    fn sync_prunes_orphan_entry_but_leaves_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let v = Vault::new(dir.path());
        sync(&v, &["by-tag"]).expect("add");
        std::fs::create_dir_all(v.layout().view_dir("by-tag")).expect("view dir");

        let report = sync(&v, &[]).expect("prune");
        assert_eq!(report.removed, ["/by-tag/"]);
        assert!(
            v.layout().view_dir("by-tag").exists(),
            "the directory must be left in place"
        );
    }

    #[test]
    fn sync_derives_multi_segment_entry() {
        let dir = tempfile::tempdir().expect("temp dir");
        let v = Vault::new(dir.path());
        let report = sync(&v, &["area/work"]).expect("sync");
        assert_eq!(report.added, ["/area/work/"]);
    }
}
