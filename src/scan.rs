// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Scanning `all-notes/` into parsed notes plus warnings (ADR 0019).
//!
//! The scan is stateless: it walks the canonical directory on every query and
//! parses frontmatter on demand (ADR 0002), with no index to keep in sync. Only
//! top-level `*.md` files are notes; non-`.md` files and any subdirectory are
//! resources and ignored silently. A malformed or badly-named top-level `.md`
//! is skipped with a warning so one bad file never breaks a query; `--strict`
//! (enforced by callers) promotes the warning set to an error.
//!
//! Traversal uses `ignore`'s parallel walker with its standard filters disabled
//! and depth pinned to the top level, so no separate parallelism crate is
//! needed (ADRs 0020, 0024).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ignore::{WalkBuilder, WalkState};

use crate::note::Note;

/// The outcome of scanning a vault's `all-notes/` directory.
#[derive(Debug, Default)]
pub struct Scan {
    /// Successfully parsed notes, ordered newest-first (ULID descending), which
    /// is the canonical default ordering (ADR 0025).
    pub notes: Vec<Note>,
    /// One entry per skipped top-level `.md` file, ordered by path for
    /// determinism.
    pub warnings: Vec<ScanWarning>,
}

/// A single skipped file and the human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanWarning {
    pub path: PathBuf,
    pub message: String,
}

/// Why a scan could not run at all (as opposed to a per-file warning).
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("the notes directory `{}` does not exist", .0.display())]
    NotesDirMissing(PathBuf),
}

impl Scan {
    /// Whether the scan produced any warnings (used to enforce `--strict`).
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Scan the given `all-notes/` directory.
pub fn scan_notes_dir(all_notes_dir: &Path) -> Result<Scan, ScanError> {
    if !all_notes_dir.is_dir() {
        return Err(ScanError::NotesDirMissing(all_notes_dir.to_path_buf()));
    }

    // The parallel walker hands files to per-thread closures, so collection
    // goes through shared, locked vectors. Contention is negligible: the lock
    // is held only to push an already-parsed note.
    let notes = Arc::new(Mutex::new(Vec::new()));
    let warnings = Arc::new(Mutex::new(Vec::new()));

    WalkBuilder::new(all_notes_dir)
        .standard_filters(false)
        .max_depth(Some(1))
        .build_parallel()
        .run(|| {
            let notes = Arc::clone(&notes);
            let warnings = Arc::clone(&warnings);
            Box::new(move |result| {
                let entry = match result {
                    Ok(entry) => entry,
                    // A traversal error on an individual entry is not fatal to
                    // the scan; skip it.
                    Err(_) => return WalkState::Continue,
                };

                // Depth 0 is the directory itself; depth-1 directories are
                // resource folders. Only depth-1 regular files can be notes.
                if entry.depth() == 0 {
                    return WalkState::Continue;
                }
                let is_file = entry.file_type().is_some_and(|ft| ft.is_file());
                if !is_file {
                    return WalkState::Continue;
                }

                let path = entry.path();
                if !is_markdown(path) {
                    // Non-`.md` resources are ignored silently.
                    return WalkState::Continue;
                }

                match load_note(path) {
                    Ok(note) => notes.lock().expect("notes lock").push(note),
                    Err(message) => warnings.lock().expect("warnings lock").push(ScanWarning {
                        path: path.to_path_buf(),
                        message,
                    }),
                }
                WalkState::Continue
            })
        });

    let mut notes = Arc::into_inner(notes)
        .expect("sole owner after walk")
        .into_inner()
        .expect("notes mutex");
    let mut warnings = Arc::into_inner(warnings)
        .expect("sole owner after walk")
        .into_inner()
        .expect("warnings mutex");

    // Parallel collection arrives in nondeterministic order; impose the
    // canonical orderings so callers and snapshots are stable.
    notes.sort_by_key(|n| std::cmp::Reverse(n.id));
    warnings.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Scan { notes, warnings })
}

/// Whether a path has a `.md` extension (case-sensitive, as on disk).
fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Read and parse one note file, returning a warning message on any failure.
fn load_note(path: &Path) -> Result<Note, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("could not read file: {e}"))?;
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    Note::parse(path.to_path_buf(), &content, modified).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    /// Create an `all-notes/` directory inside a fresh temp dir and return both.
    fn temp_notes_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let notes = dir.path().join("all-notes");
        std::fs::create_dir_all(&notes).expect("mkdir all-notes");
        (dir, notes)
    }

    fn write(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).expect("write file");
    }

    #[test]
    fn missing_notes_dir_is_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = scan_notes_dir(&dir.path().join("nope")).expect_err("missing");
        assert!(matches!(err, ScanError::NotesDirMissing(_)));
    }

    #[test]
    fn scans_valid_notes_newest_first() {
        let (_guard, notes) = temp_notes_dir();
        write(
            &notes,
            &format!("{ULID_A}-older.md"),
            "---\ntitle: Older\n---\nbody\n",
        );
        write(
            &notes,
            &format!("{ULID_B}-newer.md"),
            "---\ntitle: Newer\n---\nbody\n",
        );

        let scan = scan_notes_dir(&notes).expect("scan");
        assert_eq!(scan.notes.len(), 2);
        // ULID_B sorts after ULID_A, so it comes first (newest-first).
        assert_eq!(scan.notes[0].title, "Newer");
        assert_eq!(scan.notes[1].title, "Older");
        assert!(!scan.has_warnings());
    }

    #[test]
    fn ignores_non_md_and_subdirectories_silently() {
        let (_guard, notes) = temp_notes_dir();
        write(
            &notes,
            &format!("{ULID_A}-note.md"),
            "---\ntitle: Note\n---\n",
        );
        write(&notes, "image.png", "not markdown");
        write(&notes, "README", "plain");
        std::fs::create_dir_all(notes.join("attachments")).expect("subdir");
        write(
            &notes.join("attachments"),
            &format!("{ULID_B}-nested.md"),
            "---\ntitle: Nested\n---\n",
        );

        let scan = scan_notes_dir(&notes).expect("scan");
        assert_eq!(scan.notes.len(), 1);
        assert_eq!(scan.notes[0].title, "Note");
        // The nested `.md` is never traversed, and resources raise no warnings.
        assert!(!scan.has_warnings());
    }

    #[test]
    fn malformed_notes_become_warnings() {
        let (_guard, notes) = temp_notes_dir();
        write(
            &notes,
            &format!("{ULID_A}-good.md"),
            "---\ntitle: Good\n---\n",
        );
        // Missing title.
        write(&notes, &format!("{ULID_B}-bad.md"), "---\ntags: [x]\n---\n");
        // Badly named (no ULID prefix).
        write(&notes, "totally-wrong.md", "---\ntitle: Wrong\n---\n");

        let scan = scan_notes_dir(&notes).expect("scan");
        assert_eq!(scan.notes.len(), 1);
        assert_eq!(scan.notes[0].title, "Good");
        assert_eq!(scan.warnings.len(), 2);
        // Warnings are path-sorted: the ULID-prefixed file precedes `totally-`.
        assert!(scan.warnings[0].path.ends_with(format!("{ULID_B}-bad.md")));
        assert!(scan.warnings[1].path.ends_with("totally-wrong.md"));
    }

    #[test]
    fn empty_notes_dir_yields_nothing() {
        let (_guard, notes) = temp_notes_dir();
        let scan = scan_notes_dir(&notes).expect("scan");
        assert!(scan.notes.is_empty());
        assert!(!scan.has_warnings());
    }
}
