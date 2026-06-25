// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Selection and search: turning a query or selector into notes (ADR 0025).
//!
//! This is the headless core behind `search`, `edit` and `delete`. A *query* is
//! run through the DSL over a fresh scan; a *selector* is either a full 26-char
//! ULID (resolved directly to that one note) or, failing that, a query. Results
//! are newest-first, inherited from the scan's ordering. The picker's candidate
//! set is built here too, so the selection and match logic stays unit-testable
//! without a TTY (ADR 0021).

use std::path::PathBuf;

use crate::error::Result;
use crate::id::{Id, ULID_LEN};
use crate::note::Note;
use crate::query;
use crate::scan::{self, Scan, ScanWarning};
use crate::vault::Vault;

/// The result of a search or selection: matching notes plus scan warnings.
#[derive(Debug, Default)]
pub struct Matches {
    /// Matching notes, newest-first.
    pub notes: Vec<Note>,
    /// Warnings from the underlying scan.
    pub warnings: Vec<ScanWarning>,
}

/// A picker candidate: the fields shown in one picker row.
///
/// The binary renders these into the matchable row text and the display-only
/// ULID suffix; what is matched is decided there (ADR 0027), not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: Id,
    pub title: String,
    pub date: String,
    pub tags: Vec<String>,
    pub path: PathBuf,
}

/// Run an optional query over the vault, returning matches newest-first.
///
/// `None` (or a blank query) means all notes.
pub fn search(vault: &Vault, query: Option<&str>) -> Result<Matches> {
    let scan = scan_vault(vault)?;
    let notes = match query.map(str::trim).filter(|q| !q.is_empty()) {
        None => scan.notes,
        Some(q) => {
            let prepared = query::compile(q)?;
            scan.notes
                .into_iter()
                .filter(|n| prepared.matches(n))
                .collect()
        }
    };
    Ok(Matches {
        notes,
        warnings: scan.warnings,
    })
}

/// Resolve a `<id|query>` selector to its matching notes.
///
/// A selector that is a full 26-character ULID resolves to the single note with
/// that identity (zero or one result); anything else is treated as a query.
pub fn resolve_selection(vault: &Vault, selector: &str) -> Result<Matches> {
    match as_ulid(selector) {
        Some(id) => {
            let scan = scan_vault(vault)?;
            let notes = scan.notes.into_iter().filter(|n| n.id == id).collect();
            Ok(Matches {
                notes,
                warnings: scan.warnings,
            })
        }
        None => search(vault, Some(selector)),
    }
}

/// Parse a selector as a full ULID, or `None` if it is not one.
fn as_ulid(selector: &str) -> Option<Id> {
    if selector.len() == ULID_LEN {
        selector.parse::<Id>().ok()
    } else {
        None
    }
}

/// Build picker candidates from notes, computing each note's local date.
pub fn to_candidates(notes: &[Note]) -> Result<Vec<Candidate>> {
    notes
        .iter()
        .map(|note| {
            let date = note.created_date()?;
            Ok(Candidate {
                id: note.id,
                title: note.title.clone(),
                date,
                tags: note.tags.clone(),
                path: note.path.clone(),
            })
        })
        .collect()
}

fn scan_vault(vault: &Vault) -> Result<Scan> {
    Ok(scan::scan_notes_dir(&vault.layout().all_notes())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn temp_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let vault = Vault::new(dir.path());
        (dir, vault)
    }

    fn write(vault: &Vault, ulid: &str, slug: &str, content: &str) {
        std::fs::write(
            vault.layout().all_notes().join(format!("{ulid}-{slug}.md")),
            content,
        )
        .expect("write note");
    }

    #[test]
    fn search_without_query_returns_all_newest_first() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        write(&v, ULID_B, "b", "---\ntitle: B\n---\n");
        let m = search(&v, None).expect("search");
        assert_eq!(m.notes.len(), 2);
        // ULID_B is newer, so it leads.
        assert_eq!(m.notes[0].title, "B");
    }

    #[test]
    fn search_filters_by_query() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\ntags: [work]\n---\n");
        write(&v, ULID_B, "b", "---\ntitle: B\ntags: [home]\n---\n");
        let m = search(&v, Some("tag:work")).expect("search");
        assert_eq!(m.notes.len(), 1);
        assert_eq!(m.notes[0].title, "A");
    }

    #[test]
    fn blank_query_is_all_notes() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        assert_eq!(search(&v, Some("   ")).expect("search").notes.len(), 1);
    }

    #[test]
    fn selector_ulid_resolves_single_note() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        write(&v, ULID_B, "b", "---\ntitle: B\n---\n");
        let m = resolve_selection(&v, ULID_A).expect("resolve");
        assert_eq!(m.notes.len(), 1);
        assert_eq!(m.notes[0].id.to_string(), ULID_A);
    }

    #[test]
    fn selector_unknown_ulid_resolves_nothing() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        let m = resolve_selection(&v, ULID_B).expect("resolve");
        assert!(m.notes.is_empty());
    }

    #[test]
    fn selector_non_ulid_is_query() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\ntags: [work]\n---\n");
        let m = resolve_selection(&v, "tag:work").expect("resolve");
        assert_eq!(m.notes.len(), 1);
    }

    #[test]
    fn candidates_carry_title_tags_and_date() {
        let (_g, v) = temp_vault();
        write(
            &v,
            ULID_A,
            "a",
            "---\ntitle: Alpha\ntags: [area/work, home]\n---\n",
        );
        let m = search(&v, None).expect("search");
        let cands = to_candidates(&m.notes).expect("candidates");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].title, "Alpha");
        assert_eq!(cands[0].tags, vec!["area/work", "home"]);
        assert!(!cands[0].date.is_empty());
    }
}
