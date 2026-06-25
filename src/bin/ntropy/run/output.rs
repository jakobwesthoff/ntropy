// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Plain, machine-friendly output (ADRs 0019, 0025).
//!
//! The non-interactive note table is `id<TAB>date<TAB>title<TAB>tags<TAB>path`,
//! one note per line, no header, so `awk`/`cut` can split it; tags are
//! comma-joined within their field. Scan warnings go to stderr by file name
//! (stdout stays clean for piping). Tags (the `tags` command) print as
//! `tag<TAB>count`. The same note fields render as a human reference via
//! [`note_reference`].

use std::fmt::Display;
use std::io::Write;

use anyhow::Result;
use ntropy::note::Note;
use ntropy::ops::TagCount;
use ntropy::scan::ScanWarning;

/// Print notes as a tab-separated `id<TAB>date<TAB>title<TAB>tags<TAB>path`
/// table, newest first (ADR 0025).
pub fn print_notes(notes: &[Note]) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for note in notes {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            note.id,
            note.created_date()?,
            note.title,
            note.tags.join(","),
            note.path.display(),
        )?;
    }
    Ok(())
}

/// A human-readable note reference: `date  title  [tags]  (id)`.
///
/// The single representation used wherever a note is named to a human (delete
/// prompts and confirmations, the ambiguous-match list). Tags are dropped when
/// empty. The `id`/`date`/`title`/`tags` are taken as primitives so both a
/// [`Note`] and a picker candidate can format identically.
pub fn reference(id: impl Display, date: &str, title: &str, tags: &[String]) -> String {
    let mut s = format!("{date}  {title}");
    if !tags.is_empty() {
        s.push_str(&format!("  [{}]", tags.join(", ")));
    }
    s.push_str(&format!("  ({id})"));
    s
}

/// The [`reference`] for a parsed [`Note`], computing its local creation date.
pub fn note_reference(note: &Note) -> Result<String> {
    Ok(reference(
        note.id,
        &note.created_date()?,
        &note.title,
        &note.tags,
    ))
}

/// Print tags as a tab-separated `tag<TAB>count` table.
pub fn print_tags(tags: &[TagCount]) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for entry in tags {
        writeln!(out, "{}\t{}", entry.tag, entry.count)?;
    }
    Ok(())
}

/// Print scan warnings to stderr, one per line, identified by file name.
///
/// Only the file name is shown (not the absolute path) so the message is stable
/// and readable; the file is always a top-level entry in `all-notes/`.
pub fn print_warnings(warnings: &[ScanWarning]) {
    for warning in warnings {
        let name = warning
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| warning.path.display().to_string());
        eprintln!("warning: skipped `{name}`: {}", warning.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn reference_without_tags_omits_brackets() {
        let r = reference(ULID, "2026-06-25", "My Note", &[]);
        assert_eq!(r, "2026-06-25  My Note  (01ARZ3NDEKTSV4RRFFQ69G5FAV)");
    }

    #[test]
    fn reference_with_one_tag() {
        let r = reference(ULID, "2026-06-25", "My Note", &["work".to_string()]);
        assert_eq!(
            r,
            "2026-06-25  My Note  [work]  (01ARZ3NDEKTSV4RRFFQ69G5FAV)"
        );
    }

    #[test]
    fn reference_joins_multiple_tags_with_commas() {
        let tags = ["area/work".to_string(), "home".to_string()];
        let r = reference(ULID, "2026-06-25", "My Note", &tags);
        assert_eq!(
            r,
            "2026-06-25  My Note  [area/work, home]  (01ARZ3NDEKTSV4RRFFQ69G5FAV)"
        );
    }

    #[test]
    fn note_reference_uses_note_fields() {
        let note = Note::parse(
            std::path::PathBuf::from(format!("/v/all-notes/{ULID}-n.md")),
            "---\ntitle: Quarterly\ntags: [area/work]\n---\nbody\n",
            None,
        )
        .expect("parse");
        let r = note_reference(&note).expect("reference");
        // The date is timezone-derived, so assert the stable parts only.
        assert!(r.contains("  Quarterly  [area/work]  "));
        assert!(r.ends_with(&format!("({ULID})")));
    }
}
