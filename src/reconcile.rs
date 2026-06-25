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
use crate::note::Note;
use crate::scan::{self, ScanWarning};
use crate::vault::Vault;
use crate::view::{self, ViewDef};

/// A single filename realignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: PathBuf,
    pub to: PathBuf,
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

    let views = load_views(vault)?;
    view::rebuild_all(vault, &views, &notes)?;

    Ok(ReconcileReport {
        notes_scanned: notes.len(),
        views_rebuilt: views.len(),
        renamed,
        warnings: scan.warnings,
    })
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
