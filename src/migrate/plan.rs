// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pairing each note with where it is going (ADR 0041).
//!
//! Pure path arithmetic over an already-scanned note set, so the decision about
//! what a conversion will touch is testable without converting anything.

use std::path::{Path, PathBuf};

use super::Operation;
use crate::note::{Note, filename};

/// The suffix a rekey's freshly encrypted note wears until commit.
///
/// Rekey is the one conversion whose source and target share a filename: both
/// ends are `<ulid>.age`. Producing beside the source therefore needs a name
/// nothing else uses, and commit renames it into place.
pub const REKEY_SUFFIX: &str = ".rekey";

/// One note's journey through a conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    /// The note as it exists now.
    pub source: PathBuf,
    /// Where the converted copy is written.
    ///
    /// For a rekey this is the temporary sibling, not the final name.
    pub target: PathBuf,
    /// Where the converted copy ends up once the conversion commits.
    ///
    /// The same as `target` except for a rekey.
    pub final_target: PathBuf,
}

impl Conversion {
    /// Whether the produced file still has to be moved at commit.
    pub fn needs_rename(&self) -> bool {
        self.target != self.final_target
    }
}

/// Pair every note with where `operation` sends it.
///
/// Notes come from a scan, so anything the scanner would not treat as a note —
/// resources, subdirectories, files it could not parse — is already excluded
/// and stays untouched by the conversion.
pub fn conversions(all_notes: &Path, operation: Operation, notes: &[Note]) -> Vec<Conversion> {
    notes
        .iter()
        .map(|note| {
            let final_target = match operation {
                // The slug is re-derived from the title rather than taken from
                // the old filename, so a note whose slug had drifted comes back
                // under its current title. That makes `decrypt` a content
                // inverse of `encrypt`, not a byte-for-byte one.
                Operation::Decrypt => {
                    all_notes.join(filename::build_from_title(&note.id, &note.title))
                }
                Operation::Encrypt | Operation::Rekey => {
                    all_notes.join(filename::build_encrypted(&note.id))
                }
            };
            let target = match operation {
                Operation::Rekey => {
                    let mut name = final_target.as_os_str().to_os_string();
                    name.push(REKEY_SUFFIX);
                    PathBuf::from(name)
                }
                _ => final_target.clone(),
            };
            Conversion {
                source: note.path.clone(),
                target,
                final_target,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn note(name: &str, title: &str) -> Note {
        let content = format!("---\ntitle: {title}\n---\nbody\n");
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{name}")),
            &content,
            None,
        )
        .expect("note parses")
    }

    fn all_notes() -> &'static Path {
        Path::new("/v/all-notes")
    }

    #[test]
    fn encrypt_sends_a_markdown_note_to_its_identity_alone() {
        let notes = vec![note(
            &format!("{ULID}-quarterly-review.md"),
            "Quarterly Review",
        )];
        let plan = conversions(all_notes(), Operation::Encrypt, &notes);
        assert_eq!(plan.len(), 1);
        assert_eq!(
            plan[0].source,
            all_notes().join(format!("{ULID}-quarterly-review.md"))
        );
        assert_eq!(plan[0].target, all_notes().join(format!("{ULID}.age")));
        assert!(!plan[0].needs_rename());
    }

    #[test]
    fn decrypt_rebuilds_the_name_from_the_current_title() {
        let notes = vec![note(&format!("{ULID}.age"), "Quarterly Review")];
        let plan = conversions(all_notes(), Operation::Decrypt, &notes);
        assert_eq!(
            plan[0].target,
            all_notes().join(format!("{ULID}-quarterly-review.md"))
        );
    }

    #[test]
    fn decrypt_of_a_drifted_note_lands_under_its_title_not_its_old_name() {
        // Why `decrypt` is a content inverse of `encrypt` rather than a byte
        // one: a note whose slug had drifted before encryption comes back
        // renamed. Correct, and surprising enough to pin.
        let notes = vec![note(&format!("{ULID}.age"), "Renamed Since")];
        let plan = conversions(all_notes(), Operation::Decrypt, &notes);
        assert_eq!(
            plan[0].target,
            all_notes().join(format!("{ULID}-renamed-since.md"))
        );
    }

    #[test]
    fn rekey_produces_beside_the_source_and_renames_at_commit() {
        // Both ends of a rekey are `<ulid>.age`, so the produced file needs a
        // name of its own until the source can be removed.
        let notes = vec![note(&format!("{ULID}.age"), "Whatever")];
        let plan = conversions(all_notes(), Operation::Rekey, &notes);
        assert_eq!(plan[0].source, all_notes().join(format!("{ULID}.age")));
        assert_eq!(
            plan[0].target,
            all_notes().join(format!("{ULID}.age{REKEY_SUFFIX}"))
        );
        assert_eq!(
            plan[0].final_target,
            all_notes().join(format!("{ULID}.age"))
        );
        assert!(plan[0].needs_rename());
    }

    #[test]
    fn a_rekey_target_never_collides_with_its_source() {
        let notes = vec![note(&format!("{ULID}.age"), "Whatever")];
        let plan = conversions(all_notes(), Operation::Rekey, &notes);
        assert_ne!(plan[0].source, plan[0].target);
    }

    #[test]
    fn every_note_gets_exactly_one_conversion() {
        let notes = vec![
            note(&format!("{ULID}-a.md"), "A"),
            note("01BX5ZZKBKACTAV9WEVGEMMVRZ-b.md", "B"),
        ];
        let plan = conversions(all_notes(), Operation::Encrypt, &notes);
        assert_eq!(plan.len(), 2);
    }

    #[test]
    fn an_empty_vault_plans_nothing() {
        assert!(conversions(all_notes(), Operation::Encrypt, &[]).is_empty());
    }

    #[test]
    fn targets_are_siblings_of_their_sources() {
        // Producing beside the source keeps the rename that follows on one
        // filesystem, which is what makes it atomic.
        let notes = vec![note(&format!("{ULID}-a.md"), "A")];
        for op in [Operation::Encrypt, Operation::Decrypt, Operation::Rekey] {
            let plan = conversions(all_notes(), op, &notes);
            assert_eq!(plan[0].target.parent(), plan[0].source.parent());
        }
    }
}
