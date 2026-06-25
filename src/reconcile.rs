// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Filename realignment and view rebuilding (ADRs 0004, 0008).
//!
//! Two freshness operations live here. [`refresh_views`] is the full rebuild
//! ntropy runs after any mutation to keep views current (the deliberate v1
//! stand-in for incremental link updates). [`reconcile`] additionally realigns
//! the filenames of notes whose slug has drifted from their title, the explicit
//! catch-up after out-of-band edits. A single-note realignment ([`realign`]) is
//! exposed for the editor flow, where only the touched note is realigned so a
//! stray edit elsewhere is never renamed silently (ADR 0004).

use std::path::{Path, PathBuf};

use crate::config::PerVaultConfig;
use crate::error::Result;
use crate::fsutil;
use crate::link;
use crate::note::{Note, frontmatter};
use crate::scan::{self, ScanWarning};
use crate::vault::Vault;
use crate::view::{self, ViewDef};

/// A single filename realignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// A link target refreshed in a note body because its slug had drifted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRewrite {
    /// The note whose body was rewritten.
    pub note: PathBuf,
    /// The stale target as it appeared in the body.
    pub from: String,
    /// The refreshed target pointing at the current filename.
    pub to: String,
}

/// The outcome of a full `reconcile`.
#[derive(Debug, Default)]
pub struct ReconcileReport {
    /// Number of valid notes scanned in `all-notes/`.
    pub notes_scanned: usize,
    /// Number of materialized views rebuilt.
    pub views_rebuilt: usize,
    /// Files renamed because their slug had drifted.
    pub renamed: Vec<Rename>,
    /// Link targets refreshed to point at their notes' current filenames.
    pub links_rewritten: Vec<LinkRewrite>,
    /// Warnings from scanning `all-notes/` (malformed/badly-named files).
    pub warnings: Vec<ScanWarning>,
}

/// Rebuild all configured views from the current note set (no realignment).
///
/// Returns the scan warnings so a caller can honor `--strict`.
pub fn refresh_views(vault: &Vault) -> Result<Vec<ScanWarning>> {
    let scan = scan::scan_notes_dir(&vault.layout().all_notes())?;
    let views = load_views(vault)?;
    view::rebuild_all(vault, &views, &scan.notes)?;
    Ok(scan.warnings)
}

/// Realign drifted filenames, then rebuild all views.
pub fn reconcile(vault: &Vault) -> Result<ReconcileReport> {
    let scan = scan::scan_notes_dir(&vault.layout().all_notes())?;
    let mut notes = scan.notes;
    let mut renamed = Vec::new();

    for note in &mut notes {
        if note.slug_is_aligned() {
            continue;
        }
        let new_path = canonical_sibling(&note.path, &note.canonical_filename());
        fsutil::rename(&note.path, &new_path)?;
        renamed.push(Rename {
            from: note.path.clone(),
            to: new_path.clone(),
        });
        // Subsequent view links must target the new filename.
        note.path = new_path;
    }

    // With every filename settled, refresh stale link targets so links keep
    // resolving and stay clickable in plain Markdown viewers (ADR 0028).
    let links_rewritten = rewrite_links(&notes)?;

    let views = load_views(vault)?;
    view::rebuild_all(vault, &views, &notes)?;

    Ok(ReconcileReport {
        notes_scanned: notes.len(),
        views_rebuilt: views.len(),
        renamed,
        links_rewritten,
        warnings: scan.warnings,
    })
}

/// Rewrite stale link targets in every note body to the current filenames.
///
/// Each note is re-read so its frontmatter is preserved byte-for-byte; only the
/// body's link targets are touched, and only when at least one drifted, so an
/// up-to-date note is never rewritten.
fn rewrite_links(notes: &[Note]) -> Result<Vec<LinkRewrite>> {
    let mut rewritten = Vec::new();
    for note in notes {
        let Ok(content) = std::fs::read_to_string(&note.path) else {
            continue;
        };
        let body = frontmatter::split(&content).body;
        let body_start = content.len() - body.len();
        let Some(rewrite) = link::rewrite_body(body, notes) else {
            continue;
        };
        let mut updated = String::with_capacity(content.len());
        updated.push_str(&content[..body_start]);
        updated.push_str(&rewrite.body);
        fsutil::atomic_write(&note.path, updated.as_bytes())?;
        for change in rewrite.rewrites {
            rewritten.push(LinkRewrite {
                note: note.path.clone(),
                from: change.from,
                to: change.to,
            });
        }
    }
    Ok(rewritten)
}

/// Realign one note's filename if its slug has drifted from its title.
///
/// Best-effort and forgiving: a file that no longer parses is left untouched
/// (there is no title to realign to). Used by the editor flow on exit.
pub fn realign(path: &Path) -> Result<Option<Rename>> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let Ok(note) = Note::parse(path.to_path_buf(), &content, modified) else {
        return Ok(None);
    };
    if note.slug_is_aligned() {
        return Ok(None);
    }

    let new_path = canonical_sibling(path, &note.canonical_filename());
    fsutil::rename(path, &new_path)?;
    Ok(Some(Rename {
        from: path.to_path_buf(),
        to: new_path,
    }))
}

/// The path of `filename` in the same directory as `path`.
fn canonical_sibling(path: &Path, filename: &str) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}

/// Read the per-vault view definitions as [`ViewDef`]s.
fn load_views(vault: &Vault) -> Result<Vec<ViewDef>> {
    let config = PerVaultConfig::load(&vault.layout().config_file())?;
    Ok(config
        .views
        .iter()
        .map(|v| ViewDef::new(&v.name, &v.field))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ViewConfig;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// Build a vault with `all-notes/` and a `by-tag` view configured.
    fn vault_with_view() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");

        let mut config = PerVaultConfig::default();
        config.add(ViewConfig {
            name: "by-tag".into(),
            field: "tags".into(),
        });
        std::fs::write(
            root.join(".ntropy").join("config.toml"),
            config.to_toml().expect("toml"),
        )
        .expect("write config");

        let vault = Vault::new(root);
        (dir, vault)
    }

    fn write_note(vault: &Vault, name: &str, content: &str) -> PathBuf {
        let path = vault.layout().all_notes().join(name);
        std::fs::write(&path, content).expect("write note");
        path
    }

    #[test]
    fn refresh_builds_view_links() {
        let (_guard, vault) = vault_with_view();
        write_note(
            &vault,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );

        let warnings = refresh_views(&vault).expect("refresh");
        assert!(warnings.is_empty());

        // The link exists and resolves back to the canonical file.
        let link = vault.root().join("by-tag/area/work");
        let entries: Vec<_> = std::fs::read_dir(&link)
            .expect("read group dir")
            .map(|e| e.expect("entry").path())
            .collect();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].exists(), "symlink should resolve");
        assert!(
            std::fs::read_to_string(&entries[0])
                .expect("read via link")
                .contains("title: Note")
        );
    }

    #[test]
    fn reconcile_renames_drifted_file() {
        let (_guard, vault) = vault_with_view();
        // On-disk slug `old` no longer matches the title `Brand New`.
        let old = write_note(
            &vault,
            &format!("{ULID}-old.md"),
            "---\ntitle: Brand New\ntags: [x]\n---\nbody\n",
        );

        let report = reconcile(&vault).expect("reconcile");
        assert_eq!(report.renamed.len(), 1);
        assert!(!old.exists());
        let new = vault
            .layout()
            .all_notes()
            .join(format!("{ULID}-brand-new.md"));
        assert!(new.exists());
    }

    #[test]
    fn reconcile_leaves_aligned_files() {
        let (_guard, vault) = vault_with_view();
        write_note(
            &vault,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        let report = reconcile(&vault).expect("reconcile");
        assert!(report.renamed.is_empty());
    }

    #[test]
    fn reconcile_reports_scan_and_view_counts() {
        let (_guard, vault) = vault_with_view();
        write_note(
            &vault,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        // A second note with a missing title is skipped with a warning.
        write_note(
            &vault,
            "01BRZ3NDEKTSV4RRFFQ69G5FAV-bad.md",
            "---\ntags: [x]\n---\nbody\n",
        );
        let report = reconcile(&vault).expect("reconcile");
        assert_eq!(report.notes_scanned, 1);
        assert_eq!(report.views_rebuilt, 1);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.renamed.is_empty());
    }

    #[test]
    fn realign_only_touches_drifted_note() {
        let (_guard, vault) = vault_with_view();
        let aligned = write_note(
            &vault,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        assert!(realign(&aligned).expect("realign").is_none());

        let drifted = write_note(
            &vault,
            &format!("{ULID}-stale.md"),
            "---\ntitle: Fresh Title\n---\nbody\n",
        );
        let rename = realign(&drifted).expect("realign").expect("renamed");
        assert!(rename.to.ends_with(format!("{ULID}-fresh-title.md")));
        assert!(!drifted.exists());
    }

    const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    #[test]
    fn reconcile_rewrites_a_stale_link_target() {
        let (_guard, vault) = vault_with_view();
        write_note(
            &vault,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nbody\n",
        );
        let source = write_note(
            &vault,
            &format!("{ULID_B}-source.md"),
            &format!("---\ntitle: Source\n---\nsee [Target]({ULID}-old.md)\n"),
        );

        let report = reconcile(&vault).expect("reconcile");
        assert_eq!(report.links_rewritten.len(), 1);
        let content = std::fs::read_to_string(&source).expect("read source");
        assert!(content.contains(&format!("[Target]({ULID}-target.md)")));
    }

    #[test]
    fn reconcile_leaves_aligned_links_untouched() {
        let (_guard, vault) = vault_with_view();
        write_note(
            &vault,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nx\n",
        );
        let original = format!("---\ntitle: Source\n---\n[T]({ULID}-target.md)\n");
        let source = write_note(&vault, &format!("{ULID_B}-source.md"), &original);

        let report = reconcile(&vault).expect("reconcile");
        assert!(report.links_rewritten.is_empty());
        assert_eq!(std::fs::read_to_string(&source).expect("read"), original);
    }

    #[test]
    fn reconcile_leaves_dangling_links_untouched() {
        let (_guard, vault) = vault_with_view();
        let original = format!("---\ntitle: Source\n---\n[gone]({ULID}-missing.md)\n");
        let source = write_note(&vault, &format!("{ULID_B}-source.md"), &original);

        let report = reconcile(&vault).expect("reconcile");
        assert!(report.links_rewritten.is_empty());
        assert_eq!(std::fs::read_to_string(&source).expect("read"), original);
    }

    #[test]
    fn reconcile_renames_then_rewrites_a_self_link() {
        let (_guard, vault) = vault_with_view();
        let drifted = write_note(
            &vault,
            &format!("{ULID}-old.md"),
            &format!("---\ntitle: New Title\n---\n[self]({ULID}-old.md)\n"),
        );

        let report = reconcile(&vault).expect("reconcile");
        assert_eq!(report.renamed.len(), 1);
        assert_eq!(report.links_rewritten.len(), 1);
        assert!(!drifted.exists());
        let new = vault
            .layout()
            .all_notes()
            .join(format!("{ULID}-new-title.md"));
        let content = std::fs::read_to_string(&new).expect("read renamed");
        assert!(content.contains(&format!("[self]({ULID}-new-title.md)")));
    }

    #[test]
    fn reconcile_updates_links_to_a_renamed_note() {
        let (_guard, vault) = vault_with_view();
        // The target's slug `old` has drifted from its title `Alpha One`.
        write_note(
            &vault,
            &format!("{ULID}-old.md"),
            "---\ntitle: Alpha One\n---\nx\n",
        );
        let linker = write_note(
            &vault,
            &format!("{ULID_B}-linker.md"),
            &format!("---\ntitle: Linker\n---\n[a]({ULID}-old.md)\n"),
        );

        reconcile(&vault).expect("reconcile");
        let content = std::fs::read_to_string(&linker).expect("read linker");
        assert!(content.contains(&format!("[a]({ULID}-alpha-one.md)")));
    }

    #[test]
    fn reconcile_link_rewrite_is_idempotent() {
        let (_guard, vault) = vault_with_view();
        write_note(
            &vault,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nx\n",
        );
        write_note(
            &vault,
            &format!("{ULID_B}-source.md"),
            &format!("---\ntitle: Source\n---\n[T]({ULID}-old.md)\n"),
        );

        assert_eq!(reconcile(&vault).expect("first").links_rewritten.len(), 1);
        assert!(
            reconcile(&vault)
                .expect("second")
                .links_rewritten
                .is_empty()
        );
    }

    #[test]
    fn rebuild_prunes_stale_links() {
        let (_guard, vault) = vault_with_view();
        let path = write_note(
            &vault,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );
        refresh_views(&vault).expect("first refresh");
        assert!(vault.root().join("by-tag/area/work").is_dir());

        // Remove the note out of band, then refresh: the stale group is gone.
        std::fs::remove_file(&path).expect("remove");
        refresh_views(&vault).expect("second refresh");
        assert!(!vault.root().join("by-tag/area").exists());
    }
}
