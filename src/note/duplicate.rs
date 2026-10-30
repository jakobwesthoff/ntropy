// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Notes that share one ULID, and which of them keeps it (ADR 0067).
//!
//! Copying a note file copies the ULID in its name, so two files can claim
//! one identity. Everything downstream of the scan assumes one note per ULID,
//! so exactly one of them, the keeper, stays the note with that identity and
//! every other one is a copy.
//!
//! The keeper is decided by the file names alone: the shorter name, then the
//! one first in byte order. Copy tools append to a name (`plan-b`, `plan
//! copy`, `plan 2`), so the original usually has the shorter one. Reading only
//! names means every machine that syncs the vault picks the same keeper, and a
//! title edit, which drifts a name from its title but does not change it,
//! cannot move the identity to another file.

use std::cmp::Ordering;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::Note;

/// A note whose ULID another file keeps.
#[derive(Debug, Clone)]
pub struct Duplicate {
    /// The copy.
    pub note: Note,
    /// The file that keeps the ULID.
    pub keeper: PathBuf,
}

/// Split `notes` into one note per ULID and the copies.
///
/// The kept notes come back ordered by ULID ascending; callers impose their
/// own order. The copies are ordered by path, for stable reporting.
pub fn split(mut notes: Vec<Note>) -> (Vec<Note>, Vec<Duplicate>) {
    // Each ULID's keeper sorts first among the notes sharing it, so walking
    // the sorted list keeps the first of every run.
    notes.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| keeper_order(&a.path, &b.path)));

    let mut kept: Vec<Note> = Vec::with_capacity(notes.len());
    let mut copies = Vec::new();
    for note in notes {
        match kept.last() {
            Some(keeper) if keeper.id == note.id => copies.push(Duplicate {
                keeper: keeper.path.clone(),
                note,
            }),
            _ => kept.push(note),
        }
    }
    copies.sort_by(|a, b| a.note.path.cmp(&b.note.path));
    (kept, copies)
}

/// The keeper order of two files sharing a ULID: the one that sorts first
/// keeps it.
///
/// Two entries of one directory never have equal names, so this is a total
/// order on the files of a group.
pub fn keeper_order(a: &Path, b: &Path) -> Ordering {
    let a = name_bytes(a);
    let b = name_bytes(b);
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// The bytes of a path's file name, or of the whole path when it has none.
fn name_bytes(path: &Path) -> &[u8] {
    path.file_name().unwrap_or(path.as_os_str()).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    fn note(name: &str) -> Note {
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{name}")),
            "---\ntitle: Plan\n---\nbody\n",
            None,
        )
        .expect("note parses")
    }

    fn names(notes: &[Note]) -> Vec<String> {
        notes
            .iter()
            .map(|n| {
                n.path
                    .file_name()
                    .expect("name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn the_shorter_name_keeps_the_ulid() {
        let (kept, copies) = split(vec![
            note(&format!("{ULID}-plan copy.md")),
            note(&format!("{ULID}-plan.md")),
        ]);
        assert_eq!(names(&kept), [format!("{ULID}-plan.md")]);
        assert_eq!(copies.len(), 1);
        assert!(
            copies[0]
                .note
                .path
                .ends_with(format!("{ULID}-plan copy.md"))
        );
        assert!(copies[0].keeper.ends_with(format!("{ULID}-plan.md")));
    }

    #[test]
    fn byte_order_breaks_a_tie_in_length() {
        // The uppercase spelling of a ULID sorts before the lowercase one.
        let lower = format!("{}-plan.md", ULID.to_lowercase());
        let upper = format!("{ULID}-plan.md");
        let (kept, copies) = split(vec![note(&lower), note(&upper)]);
        assert_eq!(names(&kept), [upper]);
        assert!(copies[0].note.path.ends_with(&lower));
    }

    #[test]
    fn the_order_of_the_input_does_not_matter() {
        let files = [
            format!("{ULID}-plan-2.md"),
            format!("{ULID}-plan.md"),
            format!("{ULID}-plan-b.md"),
        ];
        let forward: Vec<Note> = files.iter().map(|f| note(f)).collect();
        let backward: Vec<Note> = files.iter().rev().map(|f| note(f)).collect();

        let (kept_forward, copies_forward) = split(forward);
        let (kept_backward, copies_backward) = split(backward);

        assert_eq!(names(&kept_forward), [format!("{ULID}-plan.md")]);
        assert_eq!(names(&kept_forward), names(&kept_backward));
        let copy_paths = |copies: &[Duplicate]| -> Vec<PathBuf> {
            copies.iter().map(|c| c.note.path.clone()).collect()
        };
        assert_eq!(copy_paths(&copies_forward), copy_paths(&copies_backward));
        assert_eq!(copies_forward.len(), 2);
        assert!(
            copies_forward
                .iter()
                .all(|c| c.keeper.ends_with(format!("{ULID}-plan.md")))
        );
    }

    #[test]
    fn notes_with_their_own_ulids_are_all_kept() {
        let (kept, copies) = split(vec![
            note(&format!("{OTHER}-other.md")),
            note(&format!("{ULID}-plan.md")),
        ]);
        assert_eq!(
            names(&kept),
            [format!("{ULID}-plan.md"), format!("{OTHER}-other.md")]
        );
        assert!(copies.is_empty());
    }

    #[test]
    fn a_path_without_a_file_name_orders_by_the_whole_path() {
        assert_eq!(
            keeper_order(Path::new("/"), Path::new("/a")),
            Ordering::Less
        );
    }
}
