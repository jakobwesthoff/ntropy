// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Selection and search: turning a query or selector into notes (ADR 0025).
//!
//! This is the headless core behind `search`, `edit`, `delete`, `render` and
//! `show`. A *query* is run through the DSL over a fresh scan; a *selector* is a
//! full 26-char ULID (resolved directly to that note), a note's filename or
//! path (ADR 0091) or, failing those, a query. Results are newest-first,
//! inherited from the scan's ordering. The picker's candidate set is built here
//! too, so the selection and match logic stays unit-testable without a TTY
//! (ADR 0021).

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::note::{Note, filename};
use crate::ops::locate::{self, Named, TargetError};
use crate::query;
use crate::scan::{self, Scan, ScanWarning};
use crate::session::VaultSession;

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
pub fn search(session: &VaultSession, query: Option<&str>) -> Result<Matches> {
    let scan = scan_vault(session)?;
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

/// What a command's trailing arguments select: a note named by its ULID,
/// filename or path, or the notes a query matches.
///
/// The arguments are kept as typed beside the query they compose. A path is
/// read from the typed argument, as composing turns an argument that holds
/// spaces into a pattern, and counts only when it is the whole selector, the
/// way a full ULID does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    query: String,
    sole: Option<String>,
}

impl Selector {
    /// The selector the arguments of one command line make.
    pub fn from_arguments(arguments: &[String]) -> Self {
        let mut typed = arguments
            .iter()
            .map(|argument| argument.trim())
            .filter(|argument| !argument.is_empty());
        let sole = match (typed.next(), typed.next()) {
            (Some(only), None) => Some(only.to_string()),
            _ => None,
        };
        Selector {
            query: query::from_arguments(arguments),
            sole,
        }
    }

    /// The arguments composed into one query string; empty without arguments.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The selector as a message names it back: the query the arguments make
    /// (ADR 0076), or a filename or path as it was typed, since composing
    /// would turn one that holds spaces into a pattern.
    pub fn shown(&self) -> &str {
        self.file_argument().unwrap_or(&self.query)
    }

    /// The sole argument when it reads as a file: it ends in `.md` or `.age`.
    fn file_argument(&self) -> Option<&str> {
        self.sole
            .as_deref()
            .filter(|argument| argument.ends_with(".md") || argument.ends_with(".age"))
    }
}

impl From<&str> for Selector {
    /// A selector made of the one argument `text`.
    fn from(text: &str) -> Self {
        Selector::from_arguments(&[text.to_string()])
    }
}

/// Resolve a `<id|path|query>` selector to its matching notes.
///
/// A selector that is a full 26-character ULID resolves to every file carrying
/// that identity: the note, plus any copy the scan skipped because the note
/// keeps the ULID (ADR 0067), so a ULID two files share stays ambiguous
/// rather than quietly meaning one of them.
///
/// A sole argument ending in `.md` or `.age` that names an existing file
/// resolves to the note in that file, and only that file, a copy included
/// (ADR 0091). A file that is no note of the vault is an error, and so is the
/// name of a note file that does not exist, since as a query it would match
/// the notes linking to it. Any other such argument is a query.
///
/// Anything else is treated as a query over the notes.
pub fn resolve_selection(session: &VaultSession, selector: &Selector) -> Result<Matches> {
    if let Some(argument) = selector.file_argument() {
        match locate::name_file(session, argument) {
            Named::Note(path) => return matches_for_file(session, &path),
            Named::NotANote => return Err(TargetError::OutsideVault(argument.to_string()).into()),
            Named::Missing => {
                // A bare name is looked up in `all-notes/` only, so a file of
                // that name beside the working directory is a file of the
                // vault's neighbourhood that is no note.
                if !locate::has_separator(argument) && Path::new(argument).is_file() {
                    return Err(TargetError::OutsideVault(argument.to_string()).into());
                }
                if locate::is_note_file_name(argument) {
                    return Err(TargetError::NotFound(argument.to_string()).into());
                }
            }
        }
    }

    let selector = selector.query();
    match locate::as_ulid(selector) {
        Some(id) => {
            let scan = scan_vault(session)?;
            let copies = scan
                .duplicates
                .into_iter()
                .map(|duplicate| duplicate.note)
                .filter(|n| n.id == id);
            let notes = scan
                .notes
                .into_iter()
                .filter(|n| n.id == id)
                .chain(copies)
                .collect();
            Ok(Matches {
                notes,
                warnings: scan.warnings,
            })
        }
        None => search(session, Some(selector)),
    }
}

/// The note stored in `path`, a file directly in `all-notes/`.
///
/// The note is found by file identity, so a path spelled in other letter case
/// or reached through a link finds the same note. The scan sets aside a copy of
/// a note, which is still the note this path names. A file the scan skipped has
/// no note: the result is empty and the warnings say why.
fn matches_for_file(session: &VaultSession, path: &Path) -> Result<Matches> {
    let scan = scan_vault(session)?;
    let wanted = fsutil::file_id(path);
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| filename::parse_any(name).ok())
        .map(|parsed| parsed.id);

    let candidates = scan
        .notes
        .into_iter()
        .chain(scan.duplicates.into_iter().map(|duplicate| duplicate.note));
    let notes = match (id, wanted) {
        (Some(id), Some(wanted)) => candidates
            .filter(|note| note.id == id)
            .find(|note| fsutil::file_id(&note.path) == Some(wanted))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    };
    Ok(Matches {
        notes,
        warnings: scan.warnings,
    })
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

fn scan_vault(session: &VaultSession) -> Result<Scan> {
    Ok(scan::scan_notes_dir(
        &session.layout().all_notes(),
        session.cipher(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn temp_vault() -> (tempfile::TempDir, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));
        (dir, session)
    }

    fn write(session: &VaultSession, ulid: &str, slug: &str, content: &str) {
        std::fs::write(
            session
                .layout()
                .all_notes()
                .join(format!("{ulid}-{slug}.md")),
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
        let m = resolve_selection(&v, &Selector::from(ULID_A)).expect("resolve");
        assert_eq!(m.notes.len(), 1);
        assert_eq!(m.notes[0].id.to_string(), ULID_A);
    }

    #[test]
    fn a_full_ulid_selector_sees_every_file_of_that_id() {
        // A query sees only the note that keeps the ULID; the ULID itself
        // names the copy as well, keeper first.
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "plan", "---\ntitle: Plan\ntags: [work]\n---\n");
        write(
            &v,
            ULID_A,
            "plan-b",
            "---\ntitle: Plan B\ntags: [work]\n---\n",
        );
        write(&v, ULID_B, "other", "---\ntitle: Other\n---\n");

        let by_ulid = resolve_selection(&v, &Selector::from(ULID_A)).expect("resolve");
        let titles: Vec<&str> = by_ulid.notes.iter().map(|n| n.title.as_str()).collect();
        assert_eq!(titles, ["Plan", "Plan B"]);

        let by_query = resolve_selection(&v, &Selector::from("tag:work")).expect("resolve");
        assert_eq!(by_query.notes.len(), 1);
    }

    #[test]
    fn selector_unknown_ulid_resolves_nothing() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        let m = resolve_selection(&v, &Selector::from(ULID_B)).expect("resolve");
        assert!(m.notes.is_empty());
    }

    #[test]
    fn selector_non_ulid_is_query() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\ntags: [work]\n---\n");
        let m = resolve_selection(&v, &Selector::from("tag:work")).expect("resolve");
        assert_eq!(m.notes.len(), 1);
    }

    fn args(arguments: &[&str]) -> Selector {
        let arguments: Vec<String> = arguments.iter().map(|a| (*a).to_string()).collect();
        Selector::from_arguments(&arguments)
    }

    fn note_path(v: &VaultSession, ulid: &str, slug: &str) -> String {
        v.layout()
            .all_notes()
            .join(format!("{ulid}-{slug}.md"))
            .to_string_lossy()
            .into_owned()
    }

    fn titles(m: &Matches) -> Vec<&str> {
        m.notes.iter().map(|n| n.title.as_str()).collect()
    }

    #[test]
    fn a_selector_made_of_one_argument_keeps_it_as_typed() {
        let selector = args(&["  /my vault/all-notes/x.md  "]);
        assert_eq!(selector.shown(), "/my vault/all-notes/x.md");
        // Composing reads the argument as a phrase, which is not what was typed.
        assert_ne!(selector.query(), selector.shown());
    }

    #[test]
    fn a_phrase_argument_shows_the_query_it_makes() {
        let selector = args(&["no such phrase"]);
        assert_eq!(selector.shown(), selector.query());
        assert_ne!(selector.shown(), "no such phrase");
    }

    #[test]
    fn a_selector_made_of_several_arguments_shows_its_query() {
        let selector = args(&["a.md", "tag:work"]);
        assert_eq!(selector.shown(), selector.query());
        assert_eq!(selector.file_argument(), None);
    }

    #[test]
    fn blank_arguments_do_not_count_toward_the_selector() {
        assert_eq!(args(&["", "x.md", "  "]).file_argument(), Some("x.md"));
        assert_eq!(args(&[]).shown(), "");
    }

    #[test]
    fn only_a_markdown_or_age_argument_reads_as_a_file() {
        assert_eq!(args(&["x.md"]).file_argument(), Some("x.md"));
        assert_eq!(args(&["x.age"]).file_argument(), Some("x.age"));
        assert_eq!(args(&["x.txt"]).file_argument(), None);
        assert_eq!(args(&["x.MD"]).file_argument(), None);
        assert_eq!(args(&["tag:work"]).file_argument(), None);
    }

    #[test]
    fn a_path_selects_exactly_its_note() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\ntags: [work]\n---\n");
        write(&v, ULID_B, "b", "---\ntitle: B\ntags: [work]\n---\n");
        let m = resolve_selection(&v, &args(&[&note_path(&v, ULID_A, "a")])).expect("resolve");
        assert_eq!(titles(&m), ["A"]);
    }

    #[test]
    fn a_bare_filename_selects_its_note() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        let m = resolve_selection(&v, &args(&[&format!("{ULID_A}-a.md")])).expect("resolve");
        assert_eq!(titles(&m), ["A"]);
    }

    #[test]
    fn a_path_with_a_space_selects_its_note() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("My Vault");
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        let v = VaultSession::plaintext(Vault::new(&root));
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        let m = resolve_selection(&v, &args(&[&note_path(&v, ULID_A, "a")])).expect("resolve");
        assert_eq!(titles(&m), ["A"]);
    }

    #[test]
    fn the_path_of_a_copy_selects_the_copy_alone() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "plan", "---\ntitle: Plan\n---\n");
        write(&v, ULID_A, "plan-b", "---\ntitle: Plan B\n---\n");
        let copy =
            resolve_selection(&v, &args(&[&note_path(&v, ULID_A, "plan-b")])).expect("resolve");
        assert_eq!(titles(&copy), ["Plan B"]);
        let keeper =
            resolve_selection(&v, &args(&[&note_path(&v, ULID_A, "plan")])).expect("resolve");
        assert_eq!(titles(&keeper), ["Plan"]);
    }

    #[test]
    fn a_file_the_scan_skips_selects_nothing_and_the_warning_says_why() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "broken", "no frontmatter at all\n");
        let m = resolve_selection(&v, &args(&[&note_path(&v, ULID_A, "broken")])).expect("resolve");
        assert!(m.notes.is_empty());
        assert_eq!(m.warnings.len(), 1);
    }

    #[test]
    fn a_file_in_all_notes_without_a_note_name_selects_nothing() {
        let (_g, v) = temp_vault();
        std::fs::write(
            v.layout().all_notes().join("meeting-minutes.md"),
            "---\ntitle: Minutes\n---\n",
        )
        .expect("write");
        let path = v.layout().all_notes().join("meeting-minutes.md");
        let m = resolve_selection(&v, &args(&[&path.to_string_lossy()])).expect("resolve");
        assert!(m.notes.is_empty());
    }

    #[test]
    fn a_file_that_is_no_note_of_the_vault_is_an_error() {
        let (g, v) = temp_vault();
        let readme = g.path().join("README.md");
        std::fs::write(&readme, "# About\n").expect("write");
        let err = resolve_selection(&v, &args(&[readme.to_str().expect("utf-8")]))
            .expect_err("not a note");
        assert!(matches!(
            err,
            crate::error::Error::Target(TargetError::OutsideVault(_))
        ));
    }

    #[test]
    fn a_missing_markdown_word_is_a_query() {
        let (_g, v) = temp_vault();
        write(
            &v,
            ULID_A,
            "a",
            "---\ntitle: A\n---\nsee notes-2026.md for details\n",
        );
        let m = resolve_selection(&v, &args(&["notes-2026.md"])).expect("resolve");
        assert_eq!(titles(&m), ["A"]);
        let none = resolve_selection(&v, &args(&["absent.md"])).expect("resolve");
        assert!(none.notes.is_empty());
    }

    #[test]
    fn a_missing_note_filename_is_an_error_not_a_query() {
        // Another note links to the old name, so the name as a query would
        // select that note.
        let (_g, v) = temp_vault();
        let stale = format!("{ULID_A}-old-title.md");
        write(
            &v,
            ULID_B,
            "linker",
            &format!("---\ntitle: Linker\n---\n[old]({stale})\n"),
        );
        for selector in [
            stale.clone(),
            format!("{ULID_A}.age"),
            note_path(&v, ULID_A, "x"),
        ] {
            let err = resolve_selection(&v, &args(&[&selector])).expect_err("no such note");
            assert!(
                matches!(err, crate::error::Error::Target(TargetError::NotFound(_))),
                "{selector}: {err:?}"
            );
        }
    }

    #[test]
    fn a_bare_name_that_exists_beside_the_working_directory_is_no_note() {
        // The tests run in the package directory, which holds `Cargo.toml`
        // and `README.md`.
        let (_g, v) = temp_vault();
        let err = resolve_selection(&v, &args(&["README.md"])).expect_err("no note");
        assert!(matches!(
            err,
            crate::error::Error::Target(TargetError::OutsideVault(_))
        ));
    }

    #[test]
    fn a_path_beside_other_arguments_is_a_query_term() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\ntags: [work]\n---\n");
        let selector = args(&[&note_path(&v, ULID_A, "a"), "tag:work"]);
        let m = resolve_selection(&v, &selector).expect("resolve");
        assert!(m.notes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_note_selects_that_note() {
        let (_g, v) = temp_vault();
        write(&v, ULID_A, "a", "---\ntitle: A\n---\n");
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let link = elsewhere.path().join("shortcut.md");
        std::os::unix::fs::symlink(note_path(&v, ULID_A, "a"), &link).expect("symlink");
        let m = resolve_selection(&v, &args(&[link.to_str().expect("utf-8")])).expect("resolve");
        assert_eq!(titles(&m), ["A"]);
    }

    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use crate::ops::create_note;
        use crate::test_support::encrypted::encrypted_vault;

        #[test]
        fn an_age_path_and_filename_select_the_note() {
            let fixture = encrypted_vault();
            let note = create_note(&fixture.session, "Sealed", None).expect("create");
            let path = note.path.to_string_lossy().into_owned();
            let name = note
                .path
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            for argument in [path, name] {
                let m = resolve_selection(&fixture.session, &args(&[&argument])).expect("resolve");
                assert_eq!(titles(&m), ["Sealed"], "{argument}");
            }
        }

        #[test]
        fn a_locked_vault_still_refuses_a_file_that_is_no_note() {
            let fixture = encrypted_vault();
            let readme = fixture.session.root().join("README.md");
            std::fs::write(&readme, "# About\n").expect("write");
            let locked = fixture.locked();
            let err = resolve_selection(&locked, &args(&[readme.to_str().expect("utf-8")]))
                .expect_err("not a note");
            assert!(matches!(
                err,
                crate::error::Error::Target(TargetError::OutsideVault(_))
            ));
        }

        #[test]
        fn a_locked_vault_cannot_read_the_note_a_path_names() {
            let fixture = encrypted_vault();
            let note = create_note(&fixture.session, "Sealed", None).expect("create");
            let locked = fixture.locked();
            let err = resolve_selection(&locked, &args(&[&note.path.to_string_lossy()]))
                .expect_err("locked");
            assert!(err.to_string().contains("locked"), "{err}");
        }
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
