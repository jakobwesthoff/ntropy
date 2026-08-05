// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Filename realignment and view syncing (ADRs 0004, 0008).
//!
//! Two freshness operations live here. [`refresh_views`] syncs the materialized
//! views to the current notes, which ntropy runs after any mutation to keep them
//! current. [`reconcile`] additionally realigns
//! the filenames of notes whose slug has drifted from their title, the explicit
//! catch-up after out-of-band edits. A single-note realignment ([`realign`]) is
//! exposed for the editor flow, where only the touched note is realigned so a
//! stray edit elsewhere is never renamed silently (ADR 0004).

use std::path::{Path, PathBuf};

use crate::config::PerVaultConfig;
use crate::error::Result;
use crate::fsutil;
use crate::gitignore;
use crate::link;
use crate::note::Note;
use crate::scan::{self, ScanWarning};
use crate::session::VaultSession;
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
    /// Number of materialized views synced.
    pub views_synced: usize,
    /// Files renamed because their slug had drifted.
    pub renamed: Vec<Rename>,
    /// Link targets refreshed to point at their notes' current filenames.
    pub links_rewritten: Vec<LinkRewrite>,
    /// Plaintext notes encrypted in place in an encrypted vault.
    pub adopted: Vec<Adopted>,
    /// `.gitignore` entries added to match the configured views.
    pub gitignore_added: Vec<String>,
    /// `.gitignore` entries pruned because their view is no longer configured.
    pub gitignore_removed: Vec<String>,
    /// Warnings from scanning `all-notes/` (malformed/badly-named files).
    pub warnings: Vec<ScanWarning>,
}

/// Sync all configured views to the current note set (no realignment).
///
/// Returns the scan warnings so a caller can honor `--strict`.
///
/// An encrypted vault has no views to sync, and returning before the scan is
/// what makes that fact load-bearing rather than cosmetic: `new` calls this
/// after creating a note, so a scan here would need the identity and creation
/// would stop working on a locked vault (ADR 0041).
pub fn refresh_views(session: &VaultSession) -> Result<Vec<ScanWarning>> {
    if session.is_encrypted() {
        return Ok(Vec::new());
    }
    let scan = scan::scan_notes_dir(&session.layout().all_notes(), session.cipher())?;
    let views = load_views(session)?;
    sync_views_and_gitignore(session, &views, &scan.notes)?;
    Ok(scan.warnings)
}

/// Sync every view and bring `.gitignore` in step with them.
///
/// The two derived-state updates that must move together — the symlink trees
/// and the ignore file — live here so every sync path shares them and they
/// cannot drift. The `view` layer stays unaware of `.gitignore`; the composition
/// is owned here.
fn sync_views_and_gitignore(
    vault: &Vault,
    views: &[ViewDef],
    notes: &[Note],
) -> Result<gitignore::SyncReport> {
    view::sync_all(vault, views, notes)?;
    let names: Vec<&str> = views.iter().map(|v| v.name.as_str()).collect();
    gitignore::sync(vault, &names)
}

/// Realign drifted filenames, adopt stray plaintext notes, then sync views.
///
/// On a locked encrypted vault only adoption runs: encrypting a note needs the
/// public recipient alone, so a machine that can create notes can also fold in
/// one that was dropped in by hand — which is exactly the machine most likely
/// to have one lying around.
pub fn reconcile(session: &VaultSession) -> Result<ReconcileReport> {
    if session.is_encrypted() && !session.is_unlocked() {
        let adopted = adopt_plaintext_notes(session)?;
        return Ok(ReconcileReport {
            adopted,
            ..Default::default()
        });
    }

    let scan = scan::scan_notes_dir(&session.layout().all_notes(), session.cipher())?;
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
    //
    // An encrypted vault's link targets cannot drift: they are derived from
    // each note's current title, and rewriting one would mean re-encrypting
    // the whole note — a fresh ciphertext and a new mtime for a body that did
    // not change, which a sync provider would dutifully upload.
    let links_rewritten = if session.is_encrypted() {
        Vec::new()
    } else {
        rewrite_links(&notes)?
    };

    // A note dropped into an encrypted vault as plaintext is the one thing the
    // vault exists to prevent, so `reconcile` folds it in rather than leaving
    // it to be warned about forever.
    let adopted = adopt_plaintext_notes(session)?;
    let mut warnings = scan.warnings;
    if !adopted.is_empty() {
        // The warnings were produced before adoption ran, so the ones it just
        // resolved would otherwise fail a `--strict` run that had, in the same
        // breath, fixed the problem.
        let fixed: Vec<&Path> = adopted.iter().map(|a| a.from.as_path()).collect();
        warnings.retain(|warning| !fixed.contains(&warning.path.as_path()));
    }

    let views = load_views(session)?;
    let (views_synced, gitignore) = if session.is_encrypted() {
        // Views are disabled here, and `.gitignore` is left alone: pruning its
        // managed entries would report "stopped ignoring /by-tag/" about a
        // directory that does not exist.
        (0, gitignore::SyncReport::default())
    } else {
        (
            views.len(),
            sync_views_and_gitignore(session, &views, &notes)?,
        )
    };

    Ok(ReconcileReport {
        notes_scanned: notes.len(),
        views_synced,
        renamed,
        links_rewritten,
        adopted,
        gitignore_added: gitignore.added,
        gitignore_removed: gitignore.removed,
        warnings,
    })
}

/// A plaintext note folded into an encrypted vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adopted {
    /// The plaintext file that was taken in and removed.
    pub from: PathBuf,
    /// The encrypted note it became.
    pub to: PathBuf,
}

/// Encrypt any well-formed plaintext note sitting in an encrypted vault.
///
/// Only files that already parse as notes are taken: a valid `<ulid>-<slug>.md`
/// name and frontmatter carrying a title. Anything else is left byte-for-byte
/// alone and stays a scan warning, exactly as it would in a plaintext vault,
/// which never renames or rewrites a file it cannot parse. Adoption fixes where
/// a note is stored, never what it says or what it is called.
fn adopt_plaintext_notes(session: &VaultSession) -> Result<Vec<Adopted>> {
    if !session.is_encrypted() {
        return Ok(Vec::new());
    }

    let all_notes = session.layout().all_notes();
    let plaintext = scan::scan_notes_dir(&all_notes, &crate::cipher::PlaintextCipher)?;

    let mut adopted = Vec::new();
    for note in plaintext.notes {
        let target = all_notes.join(crate::note::filename::build_encrypted(&note.id));
        if target.exists() {
            // Two files claiming one identity. Overwriting either would lose a
            // note, so both are left in place and the scan keeps warning.
            continue;
        }

        let content = format!("{}{}", note.raw_header, note.body);
        session.cipher().write(&target, &content)?;
        fsutil::remove_file(&note.path)?;
        adopted.push(Adopted {
            from: note.path,
            to: target,
        });
    }
    adopted.sort_by(|a, b| a.from.cmp(&b.from));
    Ok(adopted)
}

/// Rewrite stale link targets in every note body to the current filenames.
///
/// Only the body's link targets are touched, and only when at least one
/// drifted, so an up-to-date note is never rewritten. The file is rebuilt from
/// the note's retained `raw_header` and rewritten body, preserving the
/// frontmatter byte-for-byte without re-reading from disk.
fn rewrite_links(notes: &[Note]) -> Result<Vec<LinkRewrite>> {
    // Build the id index once: every note's body resolves its link targets
    // against it in O(1), instead of rescanning the whole slice per link.
    let index = link::index(notes);
    let mut rewritten = Vec::new();
    for note in notes {
        let Some(rewrite) = link::rewrite_body(&note.body, &index) else {
            continue;
        };
        let mut updated = String::with_capacity(note.raw_header.len() + rewrite.body.len());
        updated.push_str(&note.raw_header);
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
///
/// Inert in an encrypted vault, where a filename is derived from the identity
/// alone and has nothing that can drift.
pub fn realign(session: &VaultSession, path: &Path) -> Result<Option<Rename>> {
    if session.is_encrypted() {
        return Ok(None);
    }
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
    use crate::test_support::{vault_with_view, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn refresh_builds_view_links() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );

        let warnings = refresh_views(&session).expect("refresh");
        assert!(warnings.is_empty());

        // The link exists and resolves back to the canonical file.
        let link = session.root().join("by-tag/area/work");
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
        let (_guard, session) = vault_with_view();
        // On-disk slug `old` no longer matches the title `Brand New`.
        let old = write_note(
            &session,
            &format!("{ULID}-old.md"),
            "---\ntitle: Brand New\ntags: [x]\n---\nbody\n",
        );

        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.renamed.len(), 1);
        assert!(!old.exists());
        let new = session
            .layout()
            .all_notes()
            .join(format!("{ULID}-brand-new.md"));
        assert!(new.exists());
    }

    #[test]
    fn reconcile_leaves_aligned_files() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        let report = reconcile(&session).expect("reconcile");
        assert!(report.renamed.is_empty());
    }

    #[test]
    fn reconcile_reports_scan_and_view_counts() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        // A second note with a missing title is skipped with a warning.
        write_note(
            &session,
            "01BRZ3NDEKTSV4RRFFQ69G5FAV-bad.md",
            "---\ntags: [x]\n---\nbody\n",
        );
        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.notes_scanned, 1);
        assert_eq!(report.views_synced, 1);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.renamed.is_empty());
    }

    #[test]
    fn realign_only_touches_drifted_note() {
        let (_guard, session) = vault_with_view();
        let aligned = write_note(
            &session,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        assert!(realign(&session, &aligned).expect("realign").is_none());

        let drifted = write_note(
            &session,
            &format!("{ULID}-stale.md"),
            "---\ntitle: Fresh Title\n---\nbody\n",
        );
        let rename = realign(&session, &drifted)
            .expect("realign")
            .expect("renamed");
        assert!(rename.to.ends_with(format!("{ULID}-fresh-title.md")));
        assert!(!drifted.exists());
    }

    const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    #[test]
    fn reconcile_rewrites_a_stale_link_target() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nbody\n",
        );
        let source = write_note(
            &session,
            &format!("{ULID_B}-source.md"),
            &format!("---\ntitle: Source\n---\nsee [Target]({ULID}-old.md)\n"),
        );

        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.links_rewritten.len(), 1);
        let content = std::fs::read_to_string(&source).expect("read source");
        assert!(content.contains(&format!("[Target]({ULID}-target.md)")));
    }

    #[test]
    fn reconcile_preserves_frontmatter_bytes_when_rewriting() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nbody\n",
        );
        // Rich, deliberately-formatted frontmatter: every byte before the body
        // must survive the rewrite untouched.
        let header =
            "---\ntitle: Source\ntags: [area/work]\nstatus: in progress\npriority: 3\n---\n";
        let source = write_note(
            &session,
            &format!("{ULID_B}-source.md"),
            &format!("{header}see [Target]({ULID}-old.md)\n"),
        );

        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.links_rewritten.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&source).expect("read source"),
            format!("{header}see [Target]({ULID}-target.md)\n")
        );
    }

    #[test]
    fn reconcile_leaves_aligned_links_untouched() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nx\n",
        );
        let original = format!("---\ntitle: Source\n---\n[T]({ULID}-target.md)\n");
        let source = write_note(&session, &format!("{ULID_B}-source.md"), &original);

        let report = reconcile(&session).expect("reconcile");
        assert!(report.links_rewritten.is_empty());
        assert_eq!(std::fs::read_to_string(&source).expect("read"), original);
    }

    #[test]
    fn reconcile_leaves_dangling_links_untouched() {
        let (_guard, session) = vault_with_view();
        let original = format!("---\ntitle: Source\n---\n[gone]({ULID}-missing.md)\n");
        let source = write_note(&session, &format!("{ULID_B}-source.md"), &original);

        let report = reconcile(&session).expect("reconcile");
        assert!(report.links_rewritten.is_empty());
        assert_eq!(std::fs::read_to_string(&source).expect("read"), original);
    }

    #[test]
    fn reconcile_renames_then_rewrites_a_self_link() {
        let (_guard, session) = vault_with_view();
        let drifted = write_note(
            &session,
            &format!("{ULID}-old.md"),
            &format!("---\ntitle: New Title\n---\n[self]({ULID}-old.md)\n"),
        );

        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.renamed.len(), 1);
        assert_eq!(report.links_rewritten.len(), 1);
        assert!(!drifted.exists());
        let new = session
            .layout()
            .all_notes()
            .join(format!("{ULID}-new-title.md"));
        let content = std::fs::read_to_string(&new).expect("read renamed");
        assert!(content.contains(&format!("[self]({ULID}-new-title.md)")));
    }

    #[test]
    fn reconcile_updates_links_to_a_renamed_note() {
        let (_guard, session) = vault_with_view();
        // The target's slug `old` has drifted from its title `Alpha One`.
        write_note(
            &session,
            &format!("{ULID}-old.md"),
            "---\ntitle: Alpha One\n---\nx\n",
        );
        let linker = write_note(
            &session,
            &format!("{ULID_B}-linker.md"),
            &format!("---\ntitle: Linker\n---\n[a]({ULID}-old.md)\n"),
        );

        reconcile(&session).expect("reconcile");
        let content = std::fs::read_to_string(&linker).expect("read linker");
        assert!(content.contains(&format!("[a]({ULID}-alpha-one.md)")));
    }

    #[test]
    fn reconcile_link_rewrite_is_idempotent() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nx\n",
        );
        write_note(
            &session,
            &format!("{ULID_B}-source.md"),
            &format!("---\ntitle: Source\n---\n[T]({ULID}-old.md)\n"),
        );

        assert_eq!(reconcile(&session).expect("first").links_rewritten.len(), 1);
        assert!(
            reconcile(&session)
                .expect("second")
                .links_rewritten
                .is_empty()
        );
    }

    #[test]
    fn refresh_prunes_stale_links() {
        let (_guard, session) = vault_with_view();
        let path = write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );
        refresh_views(&session).expect("first refresh");
        assert!(session.root().join("by-tag/area/work").is_dir());

        // Remove the note out of band, then refresh: the stale group is gone.
        std::fs::remove_file(&path).expect("remove");
        refresh_views(&session).expect("second refresh");
        assert!(!session.root().join("by-tag/area").exists());
    }

    #[test]
    fn reconcile_adds_gitignore_entry_for_configured_view() {
        let (_guard, session) = vault_with_view();
        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.gitignore_added, ["/by-tag/"]);

        let gitignore =
            std::fs::read_to_string(session.layout().gitignore_file()).expect("read .gitignore");
        assert!(gitignore.contains("/by-tag/"), "got: {gitignore}");
    }

    #[test]
    fn reconcile_prunes_orphan_entry_but_leaves_directory() {
        let (_guard, session) = vault_with_view();
        reconcile(&session).expect("first reconcile");

        // Simulate a view removed from config out of band: a managed entry and a
        // directory remain for `old`, which is no longer configured.
        let gitignore = session.layout().gitignore_file();
        let mut content = std::fs::read_to_string(&gitignore).expect("read");
        content.push_str(&format!("{}\n/old/\n", crate::gitignore::MARKER));
        std::fs::write(&gitignore, content).expect("write");
        std::fs::create_dir_all(session.layout().view_dir("old")).expect("orphan dir");

        let report = reconcile(&session).expect("second reconcile");
        assert_eq!(report.gitignore_removed, ["/old/"]);
        assert!(
            session.layout().view_dir("old").exists(),
            "the directory must be left in place"
        );
        let after = std::fs::read_to_string(&gitignore).expect("read");
        assert!(!after.contains("/old/"), "entry not pruned: {after}");
        assert!(after.contains("/by-tag/"), "configured entry lost: {after}");
    }

    #[test]
    fn reconcile_gitignore_is_idempotent() {
        let (_guard, session) = vault_with_view();
        reconcile(&session).expect("first");
        let report = reconcile(&session).expect("second");
        assert!(report.gitignore_added.is_empty());
        assert!(report.gitignore_removed.is_empty());
    }

    /// Reconcile against a vault whose notes are encrypted.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use crate::test_support::encrypted::{encrypted_vault, write_encrypted_note};

        const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

        #[test]
        fn nothing_is_renamed_rewritten_or_touched() {
            // The combined guard against reconcile shredding an encrypted
            // vault: no rename (every name is already canonical), no link
            // rewrite, and no re-encryption — which would give an unchanged
            // note fresh ciphertext and a new mtime for a sync provider to
            // upload.
            let fixture = encrypted_vault();
            let session = &fixture.session;

            let body = format!("---\ntitle: Renamed Note\n---\nsee [T]({ULID_B}-stale.md)\n");
            let a = write_encrypted_note(session, ULID, &body);
            write_encrypted_note(session, ULID_B, "---\ntitle: Target\n---\nbody\n");

            let before = std::fs::read(&a).expect("read");
            let mtime = std::fs::metadata(&a).expect("meta").modified().ok();

            let report = reconcile(session).expect("reconcile");
            assert!(report.renamed.is_empty());
            assert!(report.links_rewritten.is_empty());
            assert_eq!(report.views_synced, 0);
            assert_eq!(std::fs::read(&a).expect("read"), before);
            assert_eq!(std::fs::metadata(&a).expect("meta").modified().ok(), mtime);
        }

        #[test]
        fn a_stray_plaintext_note_is_adopted() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let stray = write_note(
                session,
                &format!("{ULID}-hand-added.md"),
                "---\ntitle: Hand Added\n---\nbody\n",
            );

            let report = reconcile(session).expect("reconcile");
            assert_eq!(report.adopted.len(), 1);
            assert_eq!(report.adopted[0].from, stray);
            assert!(!stray.exists(), "the plaintext file must be removed");

            let encrypted = session.layout().all_notes().join(format!("{ULID}.age"));
            assert!(encrypted.is_file());
            assert_eq!(
                session.cipher().read(&encrypted).expect("read"),
                "---\ntitle: Hand Added\n---\nbody\n",
                "adoption must preserve the note byte-for-byte"
            );
        }

        #[test]
        fn adoption_clears_the_warning_it_resolved() {
            // Fix-and-still-fail in one run would be a poor experience for
            // anyone using `--strict`.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            write_note(
                session,
                &format!("{ULID}-hand-added.md"),
                "---\ntitle: Hand Added\n---\nbody\n",
            );

            let report = reconcile(session).expect("reconcile");
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        }

        #[test]
        fn adoption_is_idempotent() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            write_note(
                session,
                &format!("{ULID}-hand-added.md"),
                "---\ntitle: Hand Added\n---\nbody\n",
            );

            assert_eq!(reconcile(session).expect("first").adopted.len(), 1);
            assert!(reconcile(session).expect("second").adopted.is_empty());
        }

        #[test]
        fn adoption_works_on_a_locked_vault() {
            // Encrypting needs only the public recipient, and the machine most
            // likely to have a stray plaintext note is the one without the key.
            let fixture = encrypted_vault();
            write_note(
                &fixture.session,
                &format!("{ULID}-hand-added.md"),
                "---\ntitle: Hand Added\n---\nbody\n",
            );

            let locked = fixture.locked();
            let report = reconcile(&locked).expect("reconcile while locked");
            assert_eq!(report.adopted.len(), 1);

            // The result is readable once a key is available.
            let encrypted = fixture
                .session
                .layout()
                .all_notes()
                .join(format!("{ULID}.age"));
            assert_eq!(
                fixture.session.cipher().read(&encrypted).expect("read"),
                "---\ntitle: Hand Added\n---\nbody\n"
            );
        }

        #[test]
        fn adoption_refuses_when_the_identity_is_already_taken() {
            // Two files claiming one identity: overwriting either loses a note,
            // so both stay put and the scan keeps warning.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let encrypted =
                write_encrypted_note(session, ULID, "---\ntitle: Original\n---\nkeep\n");
            let stray = write_note(
                session,
                &format!("{ULID}-collision.md"),
                "---\ntitle: Collision\n---\nother\n",
            );

            let report = reconcile(session).expect("reconcile");
            assert!(report.adopted.is_empty());
            assert!(stray.exists(), "the stray file must be left alone");
            assert_eq!(
                session.cipher().read(&encrypted).expect("read"),
                "---\ntitle: Original\n---\nkeep\n",
                "the existing note must not be overwritten"
            );
            assert_eq!(report.warnings.len(), 1);
        }

        #[test]
        fn a_file_without_a_ulid_prefix_is_left_alone() {
            // Identical to what a plaintext vault does: reconcile never renames
            // or rewrites a file it cannot parse, and never invents an identity.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let stray = write_note(
                session,
                "meeting-notes.md",
                "---\ntitle: Meeting\n---\nbody\n",
            );
            let before = std::fs::read(&stray).expect("read");

            let report = reconcile(session).expect("reconcile");
            assert!(report.adopted.is_empty());
            assert_eq!(std::fs::read(&stray).expect("read"), before);
        }

        #[test]
        fn a_plaintext_vault_leaves_the_same_file_alone() {
            // The pairing that pins "identical to a plaintext vault".
            let (_guard, session) = vault_with_view();
            let stray = write_note(
                &session,
                "meeting-notes.md",
                "---\ntitle: Meeting\n---\nbody\n",
            );
            let before = std::fs::read(&stray).expect("read");

            let report = reconcile(&session).expect("reconcile");
            assert!(report.adopted.is_empty());
            assert!(report.renamed.is_empty());
            assert_eq!(std::fs::read(&stray).expect("read"), before);
        }

        #[test]
        fn a_file_without_a_title_is_left_alone() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let stray = write_note(session, &format!("{ULID}-no-title.md"), "no frontmatter\n");
            let before = std::fs::read(&stray).expect("read");

            let report = reconcile(session).expect("reconcile");
            assert!(report.adopted.is_empty());
            assert_eq!(std::fs::read(&stray).expect("read"), before);
            assert_eq!(report.warnings.len(), 1, "it stays warned about");
        }

        #[test]
        fn refresh_views_does_nothing_and_needs_no_key() {
            // What makes `ntropy new` work on a locked vault: this must return
            // before it would otherwise scan.
            let fixture = encrypted_vault();
            let warnings = refresh_views(&fixture.locked()).expect("refresh while locked");
            assert!(warnings.is_empty());
        }

        #[test]
        fn realign_is_inert() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let path = write_encrypted_note(session, ULID, "---\ntitle: Whatever\n---\nbody\n");
            assert!(realign(session, &path).expect("realign").is_none());
            assert!(path.exists());
        }

        #[test]
        fn gitignore_is_left_untouched() {
            // Pruning the managed entries would announce that ntropy stopped
            // ignoring a directory that does not exist.
            let fixture = encrypted_vault();
            let report = reconcile(&fixture.session).expect("reconcile");
            assert!(report.gitignore_added.is_empty());
            assert!(report.gitignore_removed.is_empty());
            assert!(!fixture.session.layout().gitignore_file().exists());
        }
    }
}
