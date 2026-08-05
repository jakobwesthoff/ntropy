// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Scanning `all-notes/` into parsed notes plus warnings (ADR 0019).
//!
//! The scan is stateless: it walks the canonical directory on every query and
//! parses frontmatter on demand (ADR 0002), with no index to keep in sync. Only
//! top-level files whose extension matches the vault's storage form are notes;
//! everything else, including any subdirectory, is a resource and ignored
//! silently. A malformed or badly-named top-level note file is skipped with a
//! warning so one bad file never breaks a query; `--strict` (enforced by
//! callers) promotes the warning set to an error.
//!
//! Reading goes through the vault's [`NoteCipher`], so an encrypted vault is
//! scanned by the same code as a plaintext one and the decryption happens
//! inside the walker's per-entry closures, in parallel with everything else
//! (ADR 0041).
//!
//! Traversal uses `ignore`'s parallel walker with its standard filters disabled
//! and depth pinned to the top level, so no separate parallelism crate is
//! needed (ADRs 0020, 0024).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ignore::{WalkBuilder, WalkState};

use crate::cipher::NoteCipher;
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

    /// The vault's notes cannot be read at all: it is locked, or this build
    /// has no encryption support.
    ///
    /// A whole-scan failure rather than a warning per note, carrying the
    /// cipher's own reason so the two cases stay distinguishable.
    #[error(transparent)]
    Unreadable(#[from] crate::cipher::CipherError),
}

impl Scan {
    /// Whether the scan produced any warnings (used to enforce `--strict`).
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Scan the given `all-notes/` directory, reading notes through `cipher`.
pub fn scan_notes_dir(all_notes_dir: &Path, cipher: &dyn NoteCipher) -> Result<Scan, ScanError> {
    if !all_notes_dir.is_dir() {
        return Err(ScanError::NotesDirMissing(all_notes_dir.to_path_buf()));
    }
    // Asked once rather than per file: a cipher that cannot read fails the
    // same way for every note, and one actionable message beats a thousand
    // identical ones.
    cipher.readable().map_err(ScanError::Unreadable)?;

    let note_ext = cipher.extension();
    // Only an encrypted vault has a reason to complain about stray Markdown:
    // there, a plaintext note in the synced directory is the very thing the
    // vault exists to prevent. In a plaintext vault a stray `.age` file is
    // just a resource, and warning about it would fire spuriously all through
    // a crashed conversion.
    let warn_about_markdown = note_ext != "md";

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
                if !has_extension(path, note_ext) {
                    if warn_about_markdown && has_extension(path, "md") {
                        warnings.lock().expect("warnings lock").push(ScanWarning {
                            path: path.to_path_buf(),
                            message:
                                "plaintext Markdown in an encrypted vault; run `ntropy reconcile` \
                                 to encrypt it in place"
                                    .to_string(),
                        });
                    }
                    // Everything else is a resource and ignored silently.
                    return WalkState::Continue;
                }

                match load_note(path, cipher) {
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

/// Whether a path carries `extension` (case-sensitive, as on disk).
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some(extension)
}

/// Read and parse one note file, returning a warning message on any failure.
///
/// Reading goes through the cipher rather than `std::fs` so an encrypted note
/// is decrypted here, in a walker thread, and never reaches the parser as
/// ciphertext. A file that fails to decrypt becomes one warning and leaves the
/// rest of the vault usable (ADR 0019).
fn load_note(path: &Path, cipher: &dyn NoteCipher) -> Result<Note, String> {
    let content = cipher.read(path).map_err(|e| e.to_string())?;
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

    /// The encrypted-vault counterparts of the plaintext cases above.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use crate::cipher::{AgeCipher, NoteCipher};
        use crate::crypto::age_io;

        /// An unlocked cipher, a locked one for the same vault, and somewhere
        /// to put notes.
        fn setup() -> (tempfile::TempDir, PathBuf, AgeCipher, AgeCipher) {
            let (guard, notes) = temp_notes_dir();
            let (identity, recipient) = age_io::generate_keypair();
            let locked = AgeCipher::new(identity.to_public(), None);
            let unlocked = AgeCipher::new(recipient, Some(identity));
            (guard, notes, unlocked, locked)
        }

        fn write_note(dir: &Path, cipher: &AgeCipher, ulid: &str, content: &str) {
            cipher
                .write(&dir.join(format!("{ulid}.age")), content)
                .expect("write encrypted note");
        }

        #[test]
        fn scans_encrypted_notes_newest_first() {
            let (_guard, notes, cipher, _) = setup();
            write_note(&notes, &cipher, ULID_A, "---\ntitle: Older\n---\nbody\n");
            write_note(&notes, &cipher, ULID_B, "---\ntitle: Newer\n---\nbody\n");

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert_eq!(scan.notes.len(), 2);
            assert_eq!(scan.notes[0].title, "Newer");
            assert_eq!(scan.notes[1].title, "Older");
            assert!(scan.warnings.is_empty());
        }

        #[test]
        fn a_locked_vault_fails_once_rather_than_warning_per_note() {
            // Three notes, one message: the actionable instruction must not be
            // buried under a warning for every file in the vault.
            let (_guard, notes, cipher, locked) = setup();
            for ulid in [ULID_A, ULID_B] {
                write_note(&notes, &cipher, ulid, "---\ntitle: T\n---\nbody\n");
            }

            assert!(matches!(
                scan_notes_dir(&notes, &locked),
                Err(ScanError::Unreadable(_))
            ));
        }

        #[test]
        fn a_locked_empty_vault_still_reports_locked() {
            // The lock is a property of the vault, not of what happens to be
            // in it, so the message does not depend on note count.
            let (_guard, notes, _, locked) = setup();
            assert!(matches!(
                scan_notes_dir(&notes, &locked),
                Err(ScanError::Unreadable(_))
            ));
        }

        #[test]
        fn plaintext_markdown_in_an_encrypted_vault_is_warned_about() {
            // A threat-model violation sitting in a synced directory, so it is
            // surfaced rather than silently treated as a resource file.
            let (_guard, notes, cipher, _) = setup();
            write_note(&notes, &cipher, ULID_A, "---\ntitle: Proper\n---\nbody\n");
            write(
                &notes,
                &format!("{ULID_B}-dropped-in.md"),
                "---\ntitle: Dropped In\n---\nbody\n",
            );

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert_eq!(scan.notes.len(), 1);
            assert_eq!(scan.warnings.len(), 1);
            assert!(
                scan.warnings[0].message.contains("ntropy reconcile"),
                "the warning must name the fix: {}",
                scan.warnings[0].message
            );
        }

        #[test]
        fn an_undecryptable_note_is_one_warning_and_the_rest_still_load() {
            // ADR 0019's robustness posture: one bad file never breaks a query.
            let (_guard, notes, cipher, _) = setup();
            write_note(&notes, &cipher, ULID_A, "---\ntitle: Fine\n---\nbody\n");
            std::fs::write(notes.join(format!("{ULID_B}.age")), b"not age ciphertext")
                .expect("write junk");

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert_eq!(scan.notes.len(), 1);
            assert_eq!(scan.notes[0].title, "Fine");
            assert_eq!(scan.warnings.len(), 1);
        }

        #[test]
        fn a_note_encrypted_to_another_key_is_one_warning() {
            let (_guard, notes, cipher, _) = setup();
            let (other_identity, other_recipient) = age_io::generate_keypair();
            let other = AgeCipher::new(other_recipient, Some(other_identity));
            write_note(&notes, &other, ULID_A, "---\ntitle: Foreign\n---\nbody\n");

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert!(scan.notes.is_empty());
            assert_eq!(scan.warnings.len(), 1);
        }

        #[test]
        fn an_encrypted_note_without_frontmatter_is_a_warning() {
            let (_guard, notes, cipher, _) = setup();
            write_note(&notes, &cipher, ULID_A, "no frontmatter here\n");

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert!(scan.notes.is_empty());
            assert_eq!(scan.warnings.len(), 1);
        }

        #[test]
        fn resources_and_subdirectories_are_still_ignored_silently() {
            let (_guard, notes, cipher, _) = setup();
            write_note(&notes, &cipher, ULID_A, "---\ntitle: Note\n---\nbody\n");
            write(&notes, "diagram.png", "not a note");
            std::fs::create_dir_all(notes.join("attachments")).expect("mkdir");
            write(
                &notes.join("attachments"),
                "nested.md",
                "---\ntitle: X\n---\n",
            );

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert_eq!(scan.notes.len(), 1);
            assert!(scan.warnings.is_empty(), "{:?}", scan.warnings);
        }

        #[test]
        fn an_empty_encrypted_vault_yields_nothing() {
            let (_guard, notes, cipher, _) = setup();
            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            assert!(scan.notes.is_empty());
            assert!(scan.warnings.is_empty());
        }

        #[test]
        fn warnings_are_ordered_by_path() {
            // Parallel collection arrives unordered; snapshots depend on this.
            let (_guard, notes, cipher, _) = setup();
            for ulid in [ULID_B, ULID_A] {
                write(
                    &notes,
                    &format!("{ulid}-stray.md"),
                    "---\ntitle: Stray\n---\n",
                );
            }

            let scan = scan_notes_dir(&notes, &cipher).expect("scan");
            let paths: Vec<_> = scan.warnings.iter().map(|w| &w.path).collect();
            let mut sorted = paths.clone();
            sorted.sort();
            assert_eq!(paths, sorted);
        }
    }

    #[test]
    fn a_stray_age_file_in_a_plaintext_vault_is_ignored_silently() {
        // The inverse warning was considered and left out: it would fire on
        // every note all through a crashed `vault decrypt`, where the
        // migration marker already blocks commands and says something useful.
        let (_guard, notes) = temp_notes_dir();
        write(
            &notes,
            &format!("{ULID_A}-real.md"),
            "---\ntitle: Real\n---\nbody\n",
        );
        write(&notes, &format!("{ULID_B}.age"), "ciphertext-ish");

        let scan = scan_notes_dir(&notes, &crate::cipher::PlaintextCipher).expect("scan");
        assert_eq!(scan.notes.len(), 1);
        assert!(scan.warnings.is_empty());
    }

    #[test]
    fn missing_notes_dir_is_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = scan_notes_dir(&dir.path().join("nope"), &crate::cipher::PlaintextCipher)
            .expect_err("missing");
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

        let scan = scan_notes_dir(&notes, &crate::cipher::PlaintextCipher).expect("scan");
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

        let scan = scan_notes_dir(&notes, &crate::cipher::PlaintextCipher).expect("scan");
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

        let scan = scan_notes_dir(&notes, &crate::cipher::PlaintextCipher).expect("scan");
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
        let scan = scan_notes_dir(&notes, &crate::cipher::PlaintextCipher).expect("scan");
        assert!(scan.notes.is_empty());
        assert!(!scan.has_warnings());
    }
}
