// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The in-memory note model.
//!
//! A [`Note`] is the parsed, query-ready representation of one canonical
//! `all-notes/*.md` file: its identity and slug from the filename (ADR 0004),
//! its recognized and arbitrary frontmatter fields (ADR 0005), and its body
//! text held in memory for `text:` search (ADR 0030). Timestamps are derived,
//! never stored: `created` from the ULID, `modified` from filesystem mtime.

pub mod filename;
pub mod frontmatter;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_yaml_ng::Mapping;

use crate::datetime::{self, DateError};
use crate::id::Id;
use crate::note::filename::FilenameError;
use crate::note::frontmatter::{Frontmatter, FrontmatterError};

/// A parsed note.
#[derive(Debug, Clone)]
pub struct Note {
    /// Canonical identity, parsed from the filename.
    pub id: Id,
    /// The slug currently in the filename (may have drifted from the title).
    pub slug: String,
    /// Canonical title from frontmatter.
    pub title: String,
    /// Normalized tags (ADR 0023).
    pub tags: Vec<String>,
    /// Raw frontmatter mapping, for generic `field:value` matching.
    pub frontmatter: Mapping,
    /// The Markdown body after the frontmatter block.
    pub body: String,
    /// The verbatim bytes preceding the body: the opening fence, the
    /// frontmatter block and the closing fence. Retained so the full file can
    /// be reconstructed (e.g. a link rewrite) without re-reading from disk and
    /// without re-serializing the frontmatter.
    pub raw_header: String,
    /// The canonical file path within `all-notes/`.
    pub path: PathBuf,
    /// Filesystem mtime, when available. Soft information (ADR 0005).
    pub modified: Option<SystemTime>,
}

/// Why a file is not a well-formed note (skipped with a warning, ADR 0019).
#[derive(Debug, thiserror::Error)]
pub enum NoteError {
    #[error("the path has no readable filename")]
    NoFilename,
    #[error(transparent)]
    Filename(#[from] FilenameError),
    #[error(transparent)]
    Frontmatter(#[from] FrontmatterError),
}

impl Note {
    /// Parse a note from its path, file contents and optional mtime.
    ///
    /// Pure with respect to the filesystem: the caller (the scanner) performs
    /// the read and stat, so this is unit-testable with synthetic inputs.
    pub fn parse(
        path: PathBuf,
        content: &str,
        modified: Option<SystemTime>,
    ) -> Result<Note, NoteError> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(NoteError::NoFilename)?;
        let parsed_name = filename::parse(name)?;

        let split = frontmatter::split(content);
        let block = split.frontmatter.ok_or(FrontmatterError::Missing)?;
        let Frontmatter {
            title,
            tags,
            mapping,
        } = frontmatter::parse_block(block)?;

        // The header is everything up to the body: reconstructing the file is
        // then `raw_header + body`, with the frontmatter bytes preserved exactly.
        let body_start = content.len() - split.body.len();
        let raw_header = content[..body_start].to_string();

        Ok(Note {
            id: parsed_name.id,
            slug: parsed_name.slug,
            title,
            tags,
            frontmatter: mapping,
            body: split.body.to_string(),
            raw_header,
            path,
            modified,
        })
    }

    /// The creation instant in epoch milliseconds, derived from the ULID.
    pub fn created_ms(&self) -> u64 {
        self.id.timestamp_ms()
    }

    /// The readable creation date (`YYYY-MM-DD`) in the system-local timezone.
    pub fn created_date(&self) -> Result<String, DateError> {
        datetime::render_local_date(self.created_ms())
    }

    /// The canonical filename this note's title currently implies.
    ///
    /// When this differs from the on-disk filename the slug has drifted and
    /// `reconcile` will rename the file (ADR 0004).
    pub fn canonical_filename(&self) -> String {
        filename::build_from_title(&self.id, &self.title)
    }

    /// Whether the on-disk slug still matches the title's slug.
    pub fn slug_is_aligned(&self) -> bool {
        self.path.file_name().and_then(|n| n.to_str()) == Some(self.canonical_filename().as_str())
    }
}

/// Build a synthetic canonical path under an `all-notes/` directory.
///
/// Shared by the create and reconcile paths so the `<dir>/<ulid>-<slug>.md`
/// convention has a single construction site.
pub fn canonical_path(all_notes_dir: &Path, id: &Id, title: &str) -> PathBuf {
    all_notes_dir.join(filename::build_from_title(id, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn note_path(slug: &str) -> PathBuf {
        PathBuf::from(format!("/vault/all-notes/{ULID}-{slug}.md"))
    }

    #[test]
    fn parse_builds_full_model() {
        let content = "---\ntitle: Quarterly Review\ntags: [area/work]\n---\nThe body.\n";
        let note = Note::parse(note_path("quarterly-review"), content, None).expect("parse");
        assert_eq!(note.id.to_string(), ULID);
        assert_eq!(note.title, "Quarterly Review");
        assert_eq!(note.tags, vec!["area/work"]);
        assert_eq!(note.body, "The body.\n");
        assert_eq!(note.slug, "quarterly-review");
    }

    #[test]
    fn parse_retains_the_raw_header_bytes() {
        let content = "---\ntitle: Quarterly Review\ntags: [area/work]\n---\nThe body.\n";
        let note = Note::parse(note_path("quarterly-review"), content, None).expect("parse");
        assert_eq!(
            note.raw_header,
            "---\ntitle: Quarterly Review\ntags: [area/work]\n---\n"
        );
        // Header and body together reconstruct the file verbatim.
        assert_eq!(format!("{}{}", note.raw_header, note.body), content);
    }

    #[test]
    fn parse_retains_a_crlf_header_verbatim() {
        let content = "---\r\ntitle: X\r\n---\r\nBody\r\n";
        let note = Note::parse(note_path("x"), content, None).expect("parse");
        assert_eq!(note.raw_header, "---\r\ntitle: X\r\n---\r\n");
        assert_eq!(format!("{}{}", note.raw_header, note.body), content);
    }

    #[test]
    fn created_ms_comes_from_ulid() {
        let note = Note::parse(note_path("x"), "---\ntitle: X\n---\n", None).expect("parse");
        let id: Id = ULID.parse().expect("valid");
        assert_eq!(note.created_ms(), id.timestamp_ms());
    }

    #[test]
    fn bad_filename_is_error() {
        let err = Note::parse(
            PathBuf::from("/vault/all-notes/not-a-note.md"),
            "---\ntitle: X\n---\n",
            None,
        )
        .expect_err("bad name");
        assert!(matches!(err, NoteError::Filename(_)));
    }

    #[test]
    fn missing_frontmatter_is_error() {
        let err = Note::parse(note_path("x"), "no frontmatter here\n", None).expect_err("no fm");
        assert!(matches!(
            err,
            NoteError::Frontmatter(FrontmatterError::Missing)
        ));
    }

    #[test]
    fn missing_title_is_error() {
        let err = Note::parse(note_path("x"), "---\ntags: [a]\n---\n", None).expect_err("no title");
        assert!(matches!(
            err,
            NoteError::Frontmatter(FrontmatterError::MissingTitle)
        ));
    }

    #[test]
    fn slug_alignment_detects_drift() {
        let aligned =
            Note::parse(note_path("aligned"), "---\ntitle: Aligned\n---\n", None).expect("parse");
        assert!(aligned.slug_is_aligned());

        let drifted = Note::parse(
            note_path("old-slug"),
            "---\ntitle: A New Title\n---\n",
            None,
        )
        .expect("parse");
        assert!(!drifted.slug_is_aligned());
        assert_eq!(
            drifted.canonical_filename(),
            format!("{ULID}-a-new-title.md")
        );
    }
}
