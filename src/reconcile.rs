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
//!
//! Both [`reconcile`] and [`realign`] also give a copied note, a second file
//! carrying another note's ULID, an identity of its own (ADR 0067).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config::PerVaultConfig;
use crate::error::Result;
use crate::fsutil;
use crate::gitignore;
use crate::id::Id;
use crate::link;
use crate::note::Note;
use crate::note::duplicate::{self, Duplicate};
use crate::note::filename;
use crate::scan::{self, ScanWarning};
use crate::session::VaultSession;
use crate::vault::{Vault, ViewName};
use crate::view::{self, SkippedView, ViewDef};

/// A single filename realignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// A copied note given a ULID of its own (ADR 0067).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reassignment {
    /// The copy under the ULID it shared.
    pub from: PathBuf,
    /// The copy under its new ULID.
    pub to: PathBuf,
    /// The file that keeps the shared ULID.
    pub keeper: PathBuf,
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
    /// Copied notes given a ULID of their own.
    pub reassigned: Vec<Reassignment>,
    /// Files renamed because their slug had drifted.
    pub renamed: Vec<Rename>,
    /// Realignments not made because a file already holds the new name; each
    /// note kept its old name. Counted as warnings (ADR 0067).
    pub refused: Vec<Rename>,
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
    /// Configured views that were not synced (ADR 0064).
    pub skipped_views: Vec<SkippedView>,
}

/// What a view refresh left for the caller to report.
#[derive(Debug, Default)]
pub struct ViewRefresh {
    /// Warnings from scanning `all-notes/`.
    pub warnings: Vec<ScanWarning>,
    /// Copies the scan left out of the views because another file keeps their
    /// ULID (ADR 0067); each also has its warning in `warnings`.
    pub duplicates: Vec<Duplicate>,
    /// Configured views that were not synced.
    pub skipped_views: Vec<SkippedView>,
}

/// Sync all configured views to the current note set (no realignment).
///
/// Returns the scan warnings so a caller can honor `--strict`, and the views
/// it skipped so a caller can report them.
///
/// An encrypted vault has no views to sync, and returning before the scan is
/// what makes that fact load-bearing rather than cosmetic: `new` calls this
/// after creating a note, so a scan here would need the identity and creation
/// would stop working on a locked vault (ADR 0041).
pub fn refresh_views(session: &VaultSession) -> Result<ViewRefresh> {
    if session.is_encrypted() {
        return Ok(ViewRefresh::default());
    }
    let scan = scan::scan_notes_dir(&session.layout().all_notes(), session.cipher())?;
    let (views, mut skipped_views) = load_views(session)?;
    let (_, unsafe_views) = sync_views_and_gitignore(session, &views, &skipped_views, &scan.notes)?;
    skipped_views.extend(unsafe_views);
    Ok(ViewRefresh {
        warnings: scan.warnings,
        duplicates: scan.duplicates,
        skipped_views,
    })
}

/// Sync every view and bring `.gitignore` in step with them.
///
/// The two derived-state updates that must move together — the symlink trees
/// and the ignore file — live here so every sync path shares them and they
/// cannot drift. The `view` layer stays unaware of `.gitignore`; the composition
/// is owned here.
///
/// Returns the views the sync skipped for an unsafe directory. Their names are
/// valid and still configured, so they keep their `.gitignore` entries.
fn sync_views_and_gitignore(
    vault: &Vault,
    views: &[ViewDef],
    skipped: &[SkippedView],
    notes: &[Note],
) -> Result<(gitignore::SyncReport, Vec<SkippedView>)> {
    let unsafe_views = view::sync_all(vault, views, notes)?;
    let gitignore = sync_gitignore(vault, views, skipped)?;
    Ok((gitignore, unsafe_views))
}

/// Bring `.gitignore` in step with the configured views.
///
/// The skipped views keep whatever entry they already have, so a view that
/// stopped syncing because of its name does not have its old directory
/// surface in `git status`.
pub fn sync_gitignore(
    vault: &Vault,
    views: &[ViewDef],
    skipped: &[SkippedView],
) -> Result<gitignore::SyncReport> {
    let names: Vec<&ViewName> = views.iter().map(|v| &v.name).collect();
    let retained: Vec<&str> = skipped.iter().map(|v| v.name.as_str()).collect();
    gitignore::sync(vault, &names, &retained)
}

/// Give copies their own ULIDs, realign drifted filenames, adopt stray
/// plaintext notes, then sync views.
///
/// On a locked encrypted vault only adoption runs: encrypting a note needs the
/// public recipient alone, so a machine that can create notes can also fold in
/// one that was dropped in by hand — which is exactly the machine most likely
/// to have one lying around.
pub fn reconcile(session: &VaultSession) -> Result<ReconcileReport> {
    if session.is_encrypted() && !session.is_unlocked() {
        let adoption = adopt_plaintext_notes(session)?;
        return Ok(ReconcileReport {
            adopted: adoption.adopted,
            reassigned: adoption.reassigned,
            ..Default::default()
        });
    }

    let all_notes = session.layout().all_notes();
    let scan = scan::scan_notes_dir(&all_notes, session.cipher())?;
    let mut notes = scan.notes;

    // Copies first: once each has its own ULID, every note's canonical name
    // is its own, and the realignment below cannot be asked to rename one
    // file onto another.
    let mut taken = taken_ids(&all_notes)?;
    let mut reassigned = Vec::new();
    for copy in scan.duplicates {
        let mut note = copy.note;
        reassigned.push(reassign(&mut note, copy.keeper, &mut taken)?);
        notes.push(note);
    }
    notes.sort_by_key(|n| std::cmp::Reverse(n.id));

    let mut renamed = Vec::new();
    let mut refused = Vec::new();

    for note in &mut notes {
        match realign_note(note)? {
            None => {}
            Some(RenameOutcome::Renamed(change)) => renamed.push(change),
            Some(RenameOutcome::Refused(change)) => refused.push(change),
        }
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
    let adoption = adopt_plaintext_notes(session)?;
    reassigned.extend(adoption.reassigned);
    let adopted = adoption.adopted;

    // The warnings were produced before the copies were reassigned and the
    // plaintext notes adopted, so the ones resolved here would otherwise fail
    // a `--strict` run that had, in the same breath, fixed the problem.
    let mut warnings = scan.warnings;
    let fixed: Vec<&Path> = adopted
        .iter()
        .map(|a| a.from.as_path())
        .chain(reassigned.iter().map(|r| r.from.as_path()))
        .collect();
    warnings.retain(|warning| !fixed.contains(&warning.path.as_path()));

    let (views, mut skipped_views) = load_views(session)?;
    let (views_synced, gitignore, skipped_views) = if session.is_encrypted() {
        // Views are disabled here, and `.gitignore` is left alone: pruning its
        // managed entries would report "stopped ignoring /by-tag/" about a
        // directory that does not exist. Nothing is synced, so nothing is
        // reported as skipped either.
        (0, gitignore::SyncReport::default(), Vec::new())
    } else {
        let (gitignore, unsafe_views) =
            sync_views_and_gitignore(session, &views, &skipped_views, &notes)?;
        let views_synced = views.len() - unsafe_views.len();
        skipped_views.extend(unsafe_views);
        (views_synced, gitignore, skipped_views)
    };

    Ok(ReconcileReport {
        notes_scanned: notes.len(),
        views_synced,
        reassigned,
        renamed,
        refused,
        links_rewritten,
        adopted,
        gitignore_added: gitignore.added,
        gitignore_removed: gitignore.removed,
        warnings,
        skipped_views,
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

/// What adopting the plaintext notes of an encrypted vault did.
#[derive(Debug, Default)]
struct Adoption {
    adopted: Vec<Adopted>,
    reassigned: Vec<Reassignment>,
}

/// Encrypt any well-formed plaintext note sitting in an encrypted vault.
///
/// Only files that already parse as notes are taken: a valid `<ulid>-<slug>.md`
/// name and frontmatter carrying a title. Anything else is left byte-for-byte
/// alone and stays a scan warning, exactly as it would in a plaintext vault,
/// which never renames or rewrites a file it cannot parse. Adoption fixes where
/// a note is stored, never what it says.
///
/// A plaintext note whose ULID an encrypted note already has, or that is a
/// copy of another plaintext note, gets a ULID of its own first (ADR 0067):
/// encrypting it under the shared one would overwrite a note. The encrypted
/// file of the shared ULID, existing or about to be adopted, keeps it. Taken
/// ULIDs come from the file names, so this works on a locked vault too.
fn adopt_plaintext_notes(session: &VaultSession) -> Result<Adoption> {
    if !session.is_encrypted() {
        return Ok(Adoption::default());
    }

    let all_notes = session.layout().all_notes();
    let plaintext = scan::scan_notes_dir(&all_notes, &crate::cipher::PlaintextCipher)?;
    let encrypted_path = |id: &Id| all_notes.join(filename::build_encrypted(id));

    let (mut conflicting, free): (Vec<Note>, Vec<Note>) = plaintext
        .notes
        .into_iter()
        .partition(|note| encrypted_path(&note.id).exists());
    conflicting.extend(plaintext.duplicates.into_iter().map(|copy| copy.note));

    let mut taken = taken_ids(&all_notes)?;
    let mut reassigned = Vec::new();
    let mut to_adopt = free;
    for mut note in conflicting {
        let keeper = encrypted_path(&note.id);
        reassigned.push(reassign(&mut note, keeper, &mut taken)?);
        to_adopt.push(note);
    }

    let mut adopted = Vec::new();
    for note in to_adopt {
        let target = encrypted_path(&note.id);
        let content = format!("{}{}", note.raw_header, note.body);
        session.cipher().write(&target, &content)?;
        fsutil::remove_file(&note.path)?;
        adopted.push(Adopted {
            from: note.path,
            to: target,
        });
    }
    adopted.sort_by(|a, b| a.from.cmp(&b.from));
    reassigned.sort_by(|a, b| a.from.cmp(&b.from));
    Ok(Adoption {
        adopted,
        reassigned,
    })
}

/// Every ULID a file in `all_notes` claims, whichever form its name has.
///
/// Read from the names alone, so the ULIDs of encrypted notes count on a
/// locked vault, and a file that does not parse as a note still holds its
/// ULID against reassignment.
fn taken_ids(all_notes: &Path) -> Result<HashSet<Id>> {
    Ok(fsutil::read_dir_entries(all_notes)?
        .into_iter()
        .filter_map(|(path, _)| {
            let name = path.file_name()?.to_str()?;
            filename::parse_any(name).ok().map(|parsed| parsed.id)
        })
        .collect())
}

/// Give `note`, a copy of the note at `keeper`, a ULID of its own and move it
/// to the canonical name for that ULID (ADR 0067).
///
/// Only the name changes: identity lives in the filename alone (ADR 0004), so
/// the content stays as it was. The new ULID is recorded in `taken`.
fn reassign(note: &mut Note, keeper: PathBuf, taken: &mut HashSet<Id>) -> Result<Reassignment> {
    let copy_name = note
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let new_id = note.id.reassigned(&copy_name, taken);
    let from = note.path.clone();
    note.id = new_id;
    let to = canonical_sibling(&from, &note.canonical_filename());
    fsutil::rename(&from, &to)?;
    taken.insert(new_id);
    note.path = to.clone();
    Ok(Reassignment { from, to, keeper })
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

/// What realigning one note's filename came to.
enum RenameOutcome {
    Renamed(Rename),
    /// A file already holds the new name, so the note kept its old one.
    Refused(Rename),
}

/// Rename `note` to its canonical filename if its slug has drifted, and point
/// `note.path` at wherever it now lives.
fn realign_note(note: &mut Note) -> Result<Option<RenameOutcome>> {
    if note.slug_is_aligned() {
        return Ok(None);
    }
    let change = Rename {
        from: note.path.clone(),
        to: canonical_sibling(&note.path, &note.canonical_filename()),
    };
    match fsutil::rename(&change.from, &change.to) {
        Ok(()) => {
            note.path = change.to.clone();
            Ok(Some(RenameOutcome::Renamed(change)))
        }
        Err(e) if e.is_destination_taken() => Ok(Some(RenameOutcome::Refused(change))),
        Err(e) => Err(e.into()),
    }
}

/// What [`realign`] did to the note it was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realignment {
    /// Where the note is now.
    pub path: PathBuf,
    /// The rename that brought its filename in line with its title.
    pub renamed: Option<Rename>,
    /// The rename that was due but not made, because a file already holds
    /// the new name. The note kept its old name; the caller warns about it.
    pub refused: Option<Rename>,
    /// Copies among the note and the other files carrying its ULID, each
    /// given a ULID of its own; the note itself may be one of them.
    pub reassigned: Vec<Reassignment>,
}

/// Realign one note's filename if its slug has drifted from its title.
///
/// Best-effort and forgiving: a file that no longer parses is left untouched
/// (there is no title to realign to). Used by the editor flow on exit and by
/// `write`, both of which have already saved the note, so a rename that is
/// refused is reported rather than failing them.
///
/// When other files carry the note's ULID, the copies among them get ULIDs of
/// their own first, by the rule the scan uses to tell the keeper (ADR 0067),
/// whether the edited note is the keeper or a copy. That also frees the
/// keeper's canonical name if a copy held it.
///
/// Inert in an encrypted vault, where a filename is derived from the identity
/// alone and has nothing that can drift.
pub fn realign(session: &VaultSession, path: &Path) -> Result<Realignment> {
    let mut realignment = Realignment {
        path: path.to_path_buf(),
        renamed: None,
        refused: None,
        reassigned: Vec::new(),
    };
    if session.is_encrypted() {
        return Ok(realignment);
    }
    let Some(note) = read_note(path) else {
        return Ok(realignment);
    };

    let all_notes = path.parent().unwrap_or_else(|| Path::new("."));
    let mut taken = taken_ids(all_notes)?;
    let mut group = same_id_siblings(path, note.id)?;
    group.push(note);
    let (mut kept, copies) = duplicate::split(group);

    for copy in copies {
        let edited = copy.note.path == path;
        let mut note = copy.note;
        realignment
            .reassigned
            .push(reassign(&mut note, copy.keeper, &mut taken)?);
        if edited {
            // A reassigned copy already has its canonical name.
            realignment.path = note.path;
        }
    }

    let mut keeper = kept
        .pop()
        .expect("the group holds at least the edited note, so it has a keeper");
    if keeper.path == path {
        match realign_note(&mut keeper)? {
            None => {}
            Some(RenameOutcome::Renamed(change)) => realignment.renamed = Some(change),
            Some(RenameOutcome::Refused(change)) => realignment.refused = Some(change),
        }
        realignment.path = keeper.path;
    }
    Ok(realignment)
}

/// Read and parse the plaintext note at `path`, or `None` when it is gone or
/// does not parse.
fn read_note(path: &Path) -> Option<Note> {
    let content = std::fs::read_to_string(path).ok()?;
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    Note::parse(path.to_path_buf(), &content, modified).ok()
}

/// The other notes in `path`'s directory that carry `id`.
///
/// Admitted by the scan's own rule, a regular file with the `.md` extension
/// that parses as a note, so `realign` sees the same copies as `reconcile`
/// and picks the same keeper.
fn same_id_siblings(path: &Path, id: Id) -> Result<Vec<Note>> {
    let all_notes = path.parent().unwrap_or_else(|| Path::new("."));
    let mut siblings = Vec::new();
    for (entry, file_type) in fsutil::read_dir_entries(all_notes)? {
        let name = entry.file_name().unwrap_or_default();
        let candidate = file_type.is_file()
            && Some(name) != path.file_name()
            && filename::has_md_extension(&entry)
            && filename::parse(&name.to_string_lossy()).is_ok_and(|parsed| parsed.id == id);
        if candidate && let Some(note) = read_note(&entry) {
            siblings.push(note);
        }
    }
    Ok(siblings)
}

/// The path of `filename` in the same directory as `path`.
fn canonical_sibling(path: &Path, filename: &str) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}

/// Read the per-vault view definitions: the valid ones as [`ViewDef`]s, the
/// ones with an invalid name as [`SkippedView`]s (ADR 0064).
fn load_views(vault: &Vault) -> Result<(Vec<ViewDef>, Vec<SkippedView>)> {
    let config = PerVaultConfig::load(&vault.layout().config_file())?;
    Ok(configured_views(&config))
}

/// Split a config's view entries into valid definitions and skipped views.
pub fn configured_views(config: &PerVaultConfig) -> (Vec<ViewDef>, Vec<SkippedView>) {
    view::parse_defs(
        config
            .views
            .iter()
            .map(|v| (v.name.as_str(), v.field.as_str())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{vault_with_view, vault_with_views, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn refresh_builds_view_links() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [area/work]\n---\nbody\n",
        );

        let warnings = refresh_views(&session).expect("refresh").warnings;
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
    fn reconcile_renames_a_note_whose_slug_dropped_a_dot() {
        let (_guard, session) = vault_with_view();
        let old = write_note(
            &session,
            &format!("{ULID}-nodejs-notes.md"),
            "---\ntitle: Node.js notes\n---\nbody\n",
        );

        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.renamed.len(), 1);
        assert!(!old.exists());
        assert!(
            session
                .layout()
                .all_notes()
                .join(format!("{ULID}-node-js-notes.md"))
                .exists()
        );
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

    // The invalid names below stay inside the temporary vault even when
    // joined onto its root, so these tests are safe against unfixed code.

    #[test]
    fn reconcile_skips_and_reports_a_view_with_an_invalid_name() {
        let (_guard, session) = vault_with_views(&[("by-tag", "tags"), ("By_Tag", "tags")]);
        let report = reconcile(&session).expect("reconcile");
        assert_eq!(report.views_synced, 1);
        let skipped: Vec<&str> = report
            .skipped_views
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        assert_eq!(skipped, ["By_Tag"]);
        assert!(!session.root().join("By_Tag").exists());
    }

    #[test]
    fn refresh_reports_a_view_with_an_invalid_name() {
        let (_guard, session) = vault_with_views(&[("By_Tag", "tags")]);
        let refresh = refresh_views(&session).expect("refresh");
        let skipped: Vec<&str> = refresh
            .skipped_views
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        assert_eq!(skipped, ["By_Tag"]);
        assert!(!session.root().join("By_Tag").exists());
    }

    #[test]
    fn a_skipped_view_keeps_its_ignore_entry() {
        // A view created under the old name rules: its tree and its managed
        // entry exist, and after the upgrade the view is skipped.
        let (_guard, session) = vault_with_views(&[("By_Tag", "tags")]);
        let gitignore = session.layout().gitignore_file();
        std::fs::write(
            &gitignore,
            format!("{}\n/By_Tag/\n", crate::gitignore::MARKER),
        )
        .expect("write");

        let report = reconcile(&session).expect("reconcile");
        assert!(report.gitignore_removed.is_empty());
        refresh_views(&session).expect("refresh");
        let after = std::fs::read_to_string(&gitignore).expect("read");
        assert!(after.contains("/By_Tag/"), "entry pruned: {after}");
    }

    #[test]
    fn realign_only_touches_drifted_note() {
        let (_guard, session) = vault_with_view();
        let aligned = write_note(
            &session,
            &format!("{ULID}-aligned.md"),
            "---\ntitle: Aligned\n---\nbody\n",
        );
        let untouched = realign(&session, &aligned).expect("realign");
        assert!(untouched.renamed.is_none());
        assert_eq!(untouched.path, aligned);

        let drifted = write_note(
            &session,
            &format!("{ULID_B}-stale.md"),
            "---\ntitle: Fresh Title\n---\nbody\n",
        );
        let realignment = realign(&session, &drifted).expect("realign");
        let rename = realignment.renamed.expect("renamed");
        assert!(rename.to.ends_with(format!("{ULID_B}-fresh-title.md")));
        assert!(aligned.exists(), "the other note is not touched");
        assert_eq!(realignment.path, rename.to);
        assert!(!drifted.exists());
    }

    /// A file holding the name `Plan` realigns to, which the scan skips
    /// because it has no frontmatter, and a note titled `Plan` under a
    /// drifted name. The returned paths are (unparseable, drifted).
    fn canonical_name_taken_by_an_unparseable_file(session: &VaultSession) -> (PathBuf, PathBuf) {
        let unparseable = write_note(session, &format!("{ULID}-plan.md"), "no frontmatter\n");
        let drifted = write_note(
            session,
            &format!("{ULID}-old.md"),
            "---\ntitle: Plan\n---\nbody\n",
        );
        (unparseable, drifted)
    }

    #[test]
    fn reconcile_does_not_rename_over_an_unparseable_file() {
        let (_guard, session) = vault_with_view();
        let (unparseable, drifted) = canonical_name_taken_by_an_unparseable_file(&session);

        let report = reconcile(&session).expect("reconcile");

        assert_eq!(
            std::fs::read_to_string(&unparseable).expect("read unparseable"),
            "no frontmatter\n",
            "the file holding the name must keep its content"
        );
        assert!(drifted.exists(), "the drifted note keeps its name");
        assert!(report.renamed.is_empty());
        assert_eq!(
            report.refused,
            [Rename {
                from: drifted,
                to: unparseable,
            }]
        );
    }

    /// The files in `all-notes/`, sorted by name.
    fn note_files(session: &VaultSession) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(session.layout().all_notes())
            .expect("read all-notes")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The one file in `all-notes/` that carries a ULID with `ULID`'s time
    /// part but not `ULID` itself: a copy's new identity.
    fn reassigned_file(session: &VaultSession) -> PathBuf {
        let time_part = &ULID[..10];
        let found: Vec<String> = note_files(session)
            .into_iter()
            .filter(|name| name.starts_with(time_part) && !name.starts_with(ULID))
            .collect();
        assert_eq!(found.len(), 1, "expected one reassigned file: {found:?}");
        session.layout().all_notes().join(&found[0])
    }

    /// A note and a copy of it made by hand, both titled `Plan`; the copy's
    /// name is the longer one. Returns (original, copy).
    fn copied_note(session: &VaultSession, tags: &str) -> (PathBuf, PathBuf) {
        let original = write_note(
            session,
            &format!("{ULID}-plan.md"),
            &format!("---\ntitle: Plan\ntags: [{tags}]\n---\noriginal\n"),
        );
        let copy = write_note(
            session,
            &format!("{ULID}-plan-b.md"),
            &format!("---\ntitle: Plan\ntags: [{tags}]\n---\ncopy\n"),
        );
        (original, copy)
    }

    #[test]
    fn reconcile_gives_a_copy_a_new_ulid() {
        let (_guard, session) = vault_with_view();
        let (original, copy) = copied_note(&session, "t");

        reconcile(&session).expect("reconcile");

        assert_eq!(
            std::fs::read_to_string(&original).expect("read original"),
            "---\ntitle: Plan\ntags: [t]\n---\noriginal\n",
            "the keeper is left as it was"
        );
        assert!(!copy.exists(), "the copy no longer carries the ULID");
        let moved = reassigned_file(&session);
        assert!(
            moved.to_string_lossy().ends_with("-plan.md"),
            "the copy is named for its title: {}",
            moved.display()
        );
        assert_eq!(
            std::fs::read_to_string(&moved).expect("read the reassigned copy"),
            "---\ntitle: Plan\ntags: [t]\n---\ncopy\n",
            "the copy's content is unchanged"
        );
    }

    #[test]
    fn reconcile_reports_the_reassignment_and_clears_its_warning() {
        let (_guard, session) = vault_with_view();
        let (original, copy) = copied_note(&session, "t");

        let report = reconcile(&session).expect("reconcile");

        assert_eq!(
            report.reassigned,
            [Reassignment {
                from: copy,
                to: reassigned_file(&session),
                keeper: original,
            }]
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(report.renamed.is_empty(), "the copy needs no second rename");
        assert_eq!(report.notes_scanned, 2);
    }

    #[test]
    fn links_to_a_reassigned_id_point_at_the_keeper() {
        // The link names the copy's old file, but identity is the ULID, and
        // the keeper is the one that kept it.
        let (_guard, session) = vault_with_view();
        copied_note(&session, "t");
        let linker = write_note(
            &session,
            &format!("{ULID_B}-linker.md"),
            &format!("---\ntitle: Linker\n---\n[plan]({ULID}-plan-b.md)\n"),
        );

        reconcile(&session).expect("reconcile");

        assert!(
            std::fs::read_to_string(&linker)
                .expect("read linker")
                .contains(&format!("[plan]({ULID}-plan.md)"))
        );
    }

    #[test]
    fn a_reassigned_copy_appears_in_the_views() {
        let (_guard, session) = vault_with_view();
        copied_note(&session, "t");

        reconcile(&session).expect("reconcile");

        let leaves = std::fs::read_dir(session.root().join("by-tag/t"))
            .expect("read the group")
            .count();
        assert_eq!(leaves, 2);
    }

    #[test]
    fn realign_gives_the_edited_copy_a_new_ulid() {
        let (_guard, session) = vault_with_view();
        let (original, copy) = copied_note(&session, "t");

        let realignment = realign(&session, &copy).expect("realign");

        assert!(original.exists(), "the keeper keeps its name");
        assert!(!copy.exists(), "the copy no longer carries the ULID");
        let moved = reassigned_file(&session);
        assert_eq!(realignment.path, moved);
        assert_eq!(
            realignment.reassigned,
            [Reassignment {
                from: copy,
                to: moved,
                keeper: original,
            }]
        );
        assert!(realignment.renamed.is_none() && realignment.refused.is_none());
    }

    #[test]
    fn realign_gives_the_other_copy_a_new_ulid_when_the_keeper_is_edited() {
        let (_guard, session) = vault_with_view();
        let (original, copy) = copied_note(&session, "t");

        let realignment = realign(&session, &original).expect("realign");

        assert!(original.exists(), "the edited keeper stays put");
        assert_eq!(realignment.path, original);
        assert!(!copy.exists(), "the copy no longer carries the ULID");
        assert_eq!(realignment.reassigned.len(), 1);
        assert_eq!(realignment.reassigned[0].from, copy);
        assert_eq!(realignment.reassigned[0].to, reassigned_file(&session));
    }

    #[test]
    fn realign_after_retitling_the_keeper_keeps_its_ulid() {
        // The keeper is retitled in the editor: its name drifts while the
        // copy's stays aligned. The keeper still keeps the ULID, and only
        // then is renamed to its new title.
        let (_guard, session) = vault_with_view();
        let original = write_note(
            &session,
            &format!("{ULID}-plan.md"),
            "---\ntitle: Plan v1\n---\noriginal\n",
        );
        let copy = write_note(
            &session,
            &format!("{ULID}-plan-v2.md"),
            "---\ntitle: Plan v2\n---\ncopy\n",
        );

        let realignment = realign(&session, &original).expect("realign");

        let retitled = session
            .layout()
            .all_notes()
            .join(format!("{ULID}-plan-v1.md"));
        assert_eq!(realignment.path, retitled);
        assert!(retitled.exists());
        assert!(!copy.exists(), "the copy no longer carries the ULID");
        assert!(
            reassigned_file(&session)
                .to_string_lossy()
                .ends_with("-plan-v2.md")
        );
    }

    #[test]
    fn realign_keeps_the_name_when_the_canonical_one_is_taken() {
        let (_guard, session) = vault_with_view();
        let (unparseable, drifted) = canonical_name_taken_by_an_unparseable_file(&session);

        let realignment = realign(&session, &drifted).expect("realign");

        assert_eq!(realignment.path, drifted);
        assert!(realignment.renamed.is_none());
        let refused = realignment.refused.expect("the rename was due");
        assert!(refused.to.ends_with(format!("{ULID}-plan.md")));
        assert_eq!(
            std::fs::read_to_string(&unparseable).expect("read unparseable"),
            "no frontmatter\n",
            "the file holding the name must keep its content"
        );
        assert!(drifted.exists(), "the drifted note keeps its name");
    }

    #[test]
    fn realign_leaves_a_missing_or_unparseable_file_alone() {
        let (_guard, session) = vault_with_view();
        let missing = session.layout().all_notes().join(format!("{ULID}-gone.md"));
        let untouched = realign(&session, &missing).expect("realign");
        assert_eq!(untouched.path, missing);
        assert!(untouched.renamed.is_none() && untouched.refused.is_none());

        let unparseable = write_note(&session, &format!("{ULID}-old.md"), "no frontmatter\n");
        let untouched = realign(&session, &unparseable).expect("realign");
        assert_eq!(untouched.path, unparseable);
        assert!(untouched.renamed.is_none() && untouched.refused.is_none());
    }

    #[test]
    fn realign_reports_a_rename_that_fails_for_another_reason() {
        // Only a taken name is a warning; a directory that cannot be written
        // is an error like any other.
        use std::os::unix::fs::PermissionsExt;

        let (_guard, session) = vault_with_view();
        let drifted = write_note(
            &session,
            &format!("{ULID}-old.md"),
            "---\ntitle: Plan\n---\nbody\n",
        );
        let all_notes = session.layout().all_notes();
        std::fs::set_permissions(&all_notes, std::fs::Permissions::from_mode(0o555))
            .expect("make all-notes read-only");

        let result = realign(&session, &drifted);

        std::fs::set_permissions(&all_notes, std::fs::Permissions::from_mode(0o755))
            .expect("restore permissions for cleanup");
        let err = result.expect_err("the rename cannot be made");
        assert!(err.to_string().contains("renaming"), "{err}");
        assert!(drifted.exists());
    }

    #[test]
    fn reconcile_renames_a_lowercase_ulid_file_to_uppercase() {
        // On a case-insensitive filesystem the uppercase name already
        // resolves to this very file, which the no-replace rule admits.
        let (_guard, session) = vault_with_view();
        let lowercase = write_note(
            &session,
            &format!("{}-note.md", ULID.to_lowercase()),
            "---\ntitle: Note\n---\nbody\n",
        );

        let report = reconcile(&session).expect("reconcile");

        assert_eq!(report.renamed.len(), 1);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let names: Vec<String> = std::fs::read_dir(lowercase.parent().expect("all-notes"))
            .expect("read all-notes")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [format!("{ULID}-note.md")]);
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
    fn reconcile_leaves_links_in_code_alone() {
        let (_guard, session) = vault_with_view();
        write_note(
            &session,
            &format!("{ULID}-target.md"),
            "---\ntitle: Target\n---\nbody\n",
        );
        let content = format!(
            "---\ntitle: Source\n---\n\
             a `span that\n[T]({ULID}-old.md) crosses a line` end\n\n\
             \x20   [T]({ULID}-old.md)\n"
        );
        let source = write_note(&session, &format!("{ULID_B}-source.md"), &content);

        let report = reconcile(&session).expect("reconcile");
        assert!(report.links_rewritten.is_empty());
        assert_eq!(
            std::fs::read_to_string(&source).expect("read source"),
            content
        );
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
        std::fs::create_dir_all(
            session
                .layout()
                .view_dir(&ViewName::parse("old").expect("valid view name")),
        )
        .expect("orphan dir");

        let report = reconcile(&session).expect("second reconcile");
        assert_eq!(report.gitignore_removed, ["/old/"]);
        assert!(
            session
                .layout()
                .view_dir(&ViewName::parse("old").expect("valid view name"))
                .exists(),
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

        /// The `.age` files in `all-notes/` other than `<ULID>.age`, read
        /// through the vault's cipher.
        fn other_encrypted_notes(session: &VaultSession) -> Vec<String> {
            let mut contents: Vec<String> = note_files(session)
                .into_iter()
                .filter(|name| name.ends_with(".age") && !name.starts_with(ULID))
                .map(|name| {
                    session
                        .cipher()
                        .read(&session.layout().all_notes().join(name))
                        .expect("read")
                })
                .collect();
            contents.sort();
            contents
        }

        #[test]
        fn adoption_reassigns_when_the_identity_is_already_taken() {
            // The encrypted note keeps the ULID; the plaintext file claiming it
            // gets one of its own and is encrypted under it, so neither is
            // overwritten and nothing is left to warn about.
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

            assert!(!stray.exists(), "the plaintext file is taken in");
            assert_eq!(
                session.cipher().read(&encrypted).expect("read"),
                "---\ntitle: Original\n---\nkeep\n",
                "the existing note must not be overwritten"
            );
            assert_eq!(
                other_encrypted_notes(session),
                ["---\ntitle: Collision\n---\nother\n"]
            );
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
            assert_eq!(report.reassigned.len(), 1);
            assert_eq!(report.reassigned[0].from, stray);
            assert_eq!(report.reassigned[0].keeper, encrypted);
            assert_eq!(report.adopted.len(), 1);
            assert_eq!(report.adopted[0].from, report.reassigned[0].to);
        }

        #[test]
        fn two_plaintext_copies_are_reassigned_and_adopted() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            write_note(
                session,
                &format!("{ULID}-plan.md"),
                "---\ntitle: Plan\n---\noriginal\n",
            );
            write_note(
                session,
                &format!("{ULID}-plan-b.md"),
                "---\ntitle: Plan\n---\ncopy\n",
            );

            let report = reconcile(session).expect("reconcile");

            let keeper = session.layout().all_notes().join(format!("{ULID}.age"));
            assert_eq!(
                session.cipher().read(&keeper).expect("read"),
                "---\ntitle: Plan\n---\noriginal\n"
            );
            assert_eq!(
                other_encrypted_notes(session),
                ["---\ntitle: Plan\n---\ncopy\n"]
            );
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
            assert_eq!(report.reassigned.len(), 1);
            assert_eq!(report.reassigned[0].keeper, keeper);
            assert_eq!(report.adopted.len(), 2);
        }

        #[test]
        fn two_plaintext_copies_are_reassigned_and_adopted_while_locked() {
            // Reassigning needs only the names in the directory, and adopting
            // only the public recipient.
            let fixture = encrypted_vault();
            write_note(
                &fixture.session,
                &format!("{ULID}-plan.md"),
                "---\ntitle: Plan\n---\noriginal\n",
            );
            write_note(
                &fixture.session,
                &format!("{ULID}-plan-b.md"),
                "---\ntitle: Plan\n---\ncopy\n",
            );

            reconcile(&fixture.locked()).expect("reconcile while locked");

            assert_eq!(
                other_encrypted_notes(&fixture.session),
                ["---\ntitle: Plan\n---\ncopy\n"]
            );
            assert!(
                note_files(&fixture.session)
                    .iter()
                    .all(|name| name.ends_with(".age")),
                "{:?}",
                note_files(&fixture.session)
            );
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
            let warnings = refresh_views(&fixture.locked())
                .expect("refresh while locked")
                .warnings;
            assert!(warnings.is_empty());
        }

        #[test]
        fn realign_is_inert() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let path = write_encrypted_note(session, ULID, "---\ntitle: Whatever\n---\nbody\n");
            let realignment = realign(session, &path).expect("realign");
            assert!(realignment.renamed.is_none() && realignment.refused.is_none());
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
