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

/// How a note is stored on disk.
///
/// Inferred from the file extension rather than carried in from the vault, so
/// [`Note::parse`] stays filesystem-pure and a note always knows which shape it
/// came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// `<ulid>-<slug>.md`: the file is the note.
    Plaintext,
    /// `<ulid>.age`: the file is ciphertext and carries no slug.
    Encrypted,
}

/// A parsed note.
#[derive(Debug, Clone)]
pub struct Note {
    /// Canonical identity, parsed from the filename.
    pub id: Id,
    /// How the note is stored on disk.
    pub storage: Storage,
    /// The note's slug.
    ///
    /// For a plaintext note this is whatever currently sits in the filename,
    /// which may have drifted from the title. An encrypted note's filename
    /// carries no slug, so it is derived from the title and therefore never
    /// drifts.
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
        let parsed_name = filename::parse_any(name)?;
        let storage = match parsed_name.slug {
            Some(_) => Storage::Plaintext,
            None => Storage::Encrypted,
        };

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

        // An encrypted note has no slug in its name, so it takes the one its
        // title implies. That is also why such a note can never be misaligned.
        let slug = parsed_name
            .slug
            .unwrap_or_else(|| crate::text::slug::slugify(&title));

        Ok(Note {
            id: parsed_name.id,
            storage,
            slug,
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

    /// The filename this note should have on disk, given how it is stored.
    ///
    /// For a plaintext note this is what its title currently implies; when it
    /// differs from the actual filename the slug has drifted and `reconcile`
    /// will rename the file (ADR 0004). For an encrypted note it is
    /// `<ulid>.age`, which never varies.
    ///
    /// Distinct from [`Note::link_target`]: what a note is *called on disk* and
    /// what other notes *write in a link to it* are the same string only in a
    /// plaintext vault, and conflating them silently corrupts one or the other.
    pub fn canonical_filename(&self) -> String {
        match self.storage {
            Storage::Plaintext => filename::build_from_title(&self.id, &self.title),
            Storage::Encrypted => filename::build_encrypted(&self.id),
        }
    }

    /// The filename other notes use when linking to this one.
    ///
    /// Always the `<ulid>-<slug>.md` form of ADR 0028, in both kinds of vault.
    /// In an encrypted vault the link target is not a real path, but nothing is
    /// lost by that: the file it would name is ciphertext, so it was never
    /// openable by a plain Markdown viewer anyway. Keeping the form means
    /// `vault encrypt` and `vault decrypt` rewrite no bodies at all, and a
    /// decrypted vault comes back with links that work.
    ///
    /// The slug it carries leaks nothing: it sits inside a note body, which is
    /// itself ciphertext, and the link's own display text names the target
    /// anyway.
    pub fn link_target(&self) -> String {
        filename::build_from_title(&self.id, &self.title)
    }

    /// Whether the on-disk filename matches what this note's storage implies.
    ///
    /// Always true for an encrypted note: its name is derived from its identity
    /// alone, so there is nothing in it that can drift. That is the whole of
    /// what "the rename machinery is inapplicable" means in practice.
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

/// Build a synthetic canonical path for a vault storing notes with
/// `extension`.
///
/// The extension comes from the vault's cipher, so `ops::create` names a new
/// note without asking which kind of vault it is writing into.
pub fn canonical_path_for(all_notes_dir: &Path, id: &Id, title: &str, extension: &str) -> PathBuf {
    all_notes_dir.join(filename::build_for(id, title, extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn note_path(slug: &str) -> PathBuf {
        PathBuf::from(format!("/vault/all-notes/{ULID}-{slug}.md"))
    }

    fn encrypted_path() -> PathBuf {
        PathBuf::from(format!("/vault/all-notes/{ULID}.age"))
    }

    // -------------------------------------------------------------------------
    // Encrypted storage
    // -------------------------------------------------------------------------

    #[test]
    fn an_age_extension_means_encrypted_storage() {
        let content = "---\ntitle: Quarterly Review\n---\nThe body.\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert_eq!(note.storage, Storage::Encrypted);
        assert_eq!(note.id.to_string(), ULID);
    }

    #[test]
    fn a_markdown_extension_means_plaintext_storage() {
        let content = "---\ntitle: Quarterly Review\n---\nThe body.\n";
        let note = Note::parse(note_path("quarterly-review"), content, None).expect("parse");
        assert_eq!(note.storage, Storage::Plaintext);
    }

    #[test]
    fn an_encrypted_note_derives_its_slug_from_the_title() {
        // There is nothing in the filename to take it from, and everything
        // downstream (render artifact names, view leaves) still wants one.
        let content = "---\ntitle: Quarterly Review\n---\nThe body.\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert_eq!(note.slug, "quarterly-review");
    }

    #[test]
    fn an_encrypted_note_with_a_yaml_special_title_still_derives_a_slug() {
        let content = "---\ntitle: \"Q3: Planning kickoff\"\n---\nBody.\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert_eq!(note.slug, "q3-planning-kickoff");
    }

    #[test]
    fn an_encrypted_note_with_an_unslugabble_title_still_parses() {
        // Whatever `slugify` does with a title made only of punctuation, the
        // note must still load: its identity is in the filename, not the slug.
        let content = "---\ntitle: \"???\"\n---\nBody.\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert_eq!(note.id.to_string(), ULID);
        assert_eq!(note.slug, crate::text::slug::slugify("???"));
    }

    #[test]
    fn an_encrypted_note_is_always_aligned() {
        // The whole of "the rename machinery is inapplicable": the name is
        // derived from the identity alone, so nothing in it can drift.
        let content = "---\ntitle: A Title That Would Drift\n---\nBody.\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert!(note.slug_is_aligned());
        assert_eq!(note.canonical_filename(), format!("{ULID}.age"));
    }

    #[test]
    fn an_encrypted_note_links_by_its_markdown_name() {
        // The naming split in one assertion: what the file is called on disk
        // and what other notes write in a link to it are different strings.
        let content = "---\ntitle: Quarterly Review\n---\nBody.\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert_eq!(note.canonical_filename(), format!("{ULID}.age"));
        assert_eq!(note.link_target(), format!("{ULID}-quarterly-review.md"));
    }

    #[test]
    fn a_plaintext_note_uses_one_name_for_both() {
        let content = "---\ntitle: Quarterly Review\n---\nBody.\n";
        let note = Note::parse(note_path("quarterly-review"), content, None).expect("parse");
        assert_eq!(note.canonical_filename(), note.link_target());
    }

    #[test]
    fn a_drifted_plaintext_note_links_by_its_current_title() {
        // Link targets follow the title, which is what makes `reconcile`'s
        // rewrite pass converge rather than chase the old filename.
        let content = "---\ntitle: Renamed Note\n---\nBody.\n";
        let note = Note::parse(note_path("old-slug"), content, None).expect("parse");
        assert!(!note.slug_is_aligned());
        assert_eq!(note.link_target(), format!("{ULID}-renamed-note.md"));
    }

    #[test]
    fn an_encrypted_note_reconstructs_verbatim() {
        // Round-tripping must survive the storage form, CRLF included.
        let content = "---\r\ntitle: Über\r\n---\r\nBody — 日本語\r\n";
        let note = Note::parse(encrypted_path(), content, None).expect("parse");
        assert_eq!(format!("{}{}", note.raw_header, note.body), content);
    }

    #[test]
    fn canonical_path_for_follows_the_vaults_extension() {
        let id: Id = ULID.parse().expect("valid");
        let dir = Path::new("/vault/all-notes");
        assert_eq!(
            canonical_path_for(dir, &id, "Quarterly Review", "md"),
            dir.join(format!("{ULID}-quarterly-review.md"))
        );
        assert_eq!(
            canonical_path_for(dir, &id, "Quarterly Review", "age"),
            dir.join(format!("{ULID}.age"))
        );
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
