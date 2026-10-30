// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Building and parsing note filenames (ADR 0004, ADR 0041).
//!
//! A plaintext vault names its notes `<ulid>-<slug>.md`. The ULID is
//! fixed-width (26 ASCII chars) and leads the name, so the split is by
//! position: the first 26 bytes are the identity, a single `-` separates it
//! from the slug, and the slug runs to the `.md` extension. Identity is read
//! from the filename and never stored in frontmatter, so this parse is the only
//! place a note's id is recovered from disk.
//!
//! An encrypted vault names its notes `<ulid>.age` and carries no slug at all,
//! because a slug is derived from the title and the title is exactly what must
//! not appear in a synced directory. The slug still exists on a parsed note;
//! it is derived from the decrypted title instead of read from the name.

use std::path::Path;

use crate::id::{Id, ULID_LEN};
use crate::text::slug;

/// The plaintext note file extension.
pub const MD_EXT: &str = ".md";

/// Whether `path` names a Markdown file: its extension is exactly `md`.
///
/// Lowercase only, the way ntropy writes notes and the way a template name
/// becomes `<name>.md`, so the answer does not depend on whether the
/// filesystem folds case.
pub fn has_md_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext == MD_EXT.trim_start_matches('.'))
}
/// The encrypted note file extension.
pub const AGE_EXT: &str = ".age";

/// A filename decomposed into its identity and (possibly drifted) slug.
///
/// The slug is whatever currently sits in the filename, which may no longer
/// match the title's slug after an out-of-band edit; realigning it is
/// `reconcile`'s job (ADR 0004), not this parser's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFilename {
    pub id: Id,
    pub slug: String,
}

/// A filename decomposed into its identity and, where the name carries one,
/// its slug.
///
/// An encrypted note's name has no slug component, which is what `None`
/// records. Callers derive the slug from the decrypted title instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    pub id: Id,
    pub slug: Option<String>,
}

/// Why a top-level filename is not a canonical note name.
#[derive(Debug, thiserror::Error)]
pub enum FilenameError {
    #[error("`{0}` does not have a .md extension")]
    NotMarkdown(String),
    #[error("`{0}` does not have a .md or .age extension")]
    NotANote(String),
    #[error("`{0}` is too short to be a <ulid>-<slug>.md name")]
    TooShort(String),
    #[error("`{0}` is missing the `-` separator after the ULID")]
    MissingSeparator(String),
    #[error("`{0}` is not a bare <ulid>.age name")]
    SluggedEncryptedName(String),
    #[error("`{name}` does not start with a valid ULID")]
    Id {
        name: String,
        #[source]
        source: crate::id::IdError,
    },
}

/// Build the canonical plaintext filename for an identity and slug.
pub fn build(id: &Id, slug: &str) -> String {
    format!("{id}-{slug}{MD_EXT}")
}

/// Build the canonical plaintext filename, deriving the slug from a title.
pub fn build_from_title(id: &Id, title: &str) -> String {
    build(id, &slug::slugify(title))
}

/// Build the canonical encrypted filename: the identity and nothing else.
pub fn build_encrypted(id: &Id) -> String {
    format!("{id}{AGE_EXT}")
}

/// Build the canonical filename for a vault storing notes with `extension`.
///
/// `extension` comes from the vault's cipher, so callers name a file without
/// having to ask which kind of vault they are writing into. An unrecognized
/// extension falls back to the plaintext form, which is the shape every vault
/// had before encryption existed.
pub fn build_for(id: &Id, title: &str, extension: &str) -> String {
    if extension == AGE_EXT.trim_start_matches('.') {
        build_encrypted(id)
    } else {
        build_from_title(id, title)
    }
}

/// Parse a note filename of either storage form.
///
/// The extension decides which shape is expected, so a slugged `.age` name or
/// a bare `<ulid>.md` is rejected rather than quietly accepted: both would mean
/// the file was named by something other than ntropy, and guessing at intent
/// there risks writing a note back under the wrong name.
pub fn parse_any(name: &str) -> Result<ParsedName, FilenameError> {
    if let Some(stem) = name.strip_suffix(AGE_EXT) {
        if stem.len() != ULID_LEN {
            return Err(if stem.len() < ULID_LEN {
                FilenameError::TooShort(name.to_string())
            } else {
                FilenameError::SluggedEncryptedName(name.to_string())
            });
        }
        let id = stem.parse::<Id>().map_err(|source| FilenameError::Id {
            name: name.to_string(),
            source,
        })?;
        return Ok(ParsedName { id, slug: None });
    }

    if name.ends_with(MD_EXT) {
        let parsed = parse(name)?;
        return Ok(ParsedName {
            id: parsed.id,
            slug: Some(parsed.slug),
        });
    }

    Err(FilenameError::NotANote(name.to_string()))
}

/// Parse a filename into its identity and slug.
pub fn parse(name: &str) -> Result<ParsedFilename, FilenameError> {
    let stem = name
        .strip_suffix(MD_EXT)
        .ok_or_else(|| FilenameError::NotMarkdown(name.to_string()))?;

    // The shortest legal stem is a full ULID, a separator and at least one slug
    // character.
    if stem.len() < ULID_LEN + 2 {
        return Err(FilenameError::TooShort(name.to_string()));
    }

    // A split inside a multibyte character means a non-ASCII byte sits within
    // the first 26, and a ULID is 26 ASCII characters, so the name cannot
    // start with one. The stem is at least 28 bytes here and a ULID exactly
    // 26, so parsing it whole is certain to fail and yields the error source.
    let Some((id_part, rest)) = stem.split_at_checked(ULID_LEN) else {
        return Err(FilenameError::Id {
            name: name.to_string(),
            source: stem
                .parse::<Id>()
                .expect_err("a stem longer than 26 bytes is never a ULID"),
        });
    };
    let id = id_part.parse::<Id>().map_err(|source| FilenameError::Id {
        name: name.to_string(),
        source,
    })?;

    let slug = rest
        .strip_prefix('-')
        .ok_or_else(|| FilenameError::MissingSeparator(name.to_string()))?;

    Ok(ParsedFilename {
        id,
        slug: slug.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn build_composes_ulid_slug_and_extension() {
        let id: Id = ULID.parse().expect("valid");
        assert_eq!(build(&id, "my-note"), format!("{ULID}-my-note.md"));
    }

    #[test]
    fn build_from_title_slugifies() {
        let id: Id = ULID.parse().expect("valid");
        assert_eq!(
            build_from_title(&id, "My Note!"),
            format!("{ULID}-my-note.md")
        );
    }

    #[test]
    fn build_encrypted_is_the_identity_and_nothing_else() {
        // Nothing title-derived may reach the filename: the name is what a
        // sync provider sees.
        let id: Id = ULID.parse().expect("valid");
        assert_eq!(build_encrypted(&id), format!("{ULID}.age"));
    }

    #[test]
    fn build_for_dispatches_on_the_vaults_extension() {
        let id: Id = ULID.parse().expect("valid");
        assert_eq!(
            build_for(&id, "My Note!", "md"),
            format!("{ULID}-my-note.md")
        );
        assert_eq!(build_for(&id, "My Note!", "age"), format!("{ULID}.age"));
    }

    #[test]
    fn parse_any_reads_a_plaintext_name_with_its_slug() {
        let id: Id = ULID.parse().expect("valid");
        let parsed = parse_any(&format!("{ULID}-quarterly-review.md")).expect("parse");
        assert_eq!(parsed.id, id);
        assert_eq!(parsed.slug.as_deref(), Some("quarterly-review"));
    }

    #[test]
    fn parse_any_reads_an_encrypted_name_with_no_slug() {
        let id: Id = ULID.parse().expect("valid");
        let parsed = parse_any(&format!("{ULID}.age")).expect("parse");
        assert_eq!(parsed.id, id);
        assert_eq!(parsed.slug, None);
    }

    #[test]
    fn parse_any_rejects_a_slugged_encrypted_name() {
        // Nothing ntropy writes looks like this, so accepting it would mean
        // guessing at what some other tool intended.
        assert!(matches!(
            parse_any(&format!("{ULID}-quarterly-review.age")),
            Err(FilenameError::SluggedEncryptedName(_))
        ));
    }

    #[test]
    fn parse_any_rejects_a_short_encrypted_name() {
        assert!(matches!(
            parse_any("too-short.age"),
            Err(FilenameError::TooShort(_))
        ));
    }

    #[test]
    fn parse_any_rejects_a_bad_ulid_in_an_encrypted_name() {
        // Right length, wrong alphabet: `U` is not in Crockford base32.
        assert!(matches!(
            parse_any("UUUUUUUUUUUUUUUUUUUUUUUUUU.age"),
            Err(FilenameError::Id { .. })
        ));
    }

    #[test]
    fn parse_any_rejects_a_name_that_is_neither_form() {
        assert!(matches!(
            parse_any("attachment.png"),
            Err(FilenameError::NotANote(_))
        ));
    }

    #[test]
    fn parse_any_rejects_a_bare_ulid_markdown_name() {
        // The `.md` branch still demands a slug, so a name ntropy would never
        // write is not silently adopted.
        assert!(matches!(
            parse_any(&format!("{ULID}.md")),
            Err(FilenameError::TooShort(_))
        ));
    }

    #[test]
    fn parse_roundtrips_build() {
        let id: Id = ULID.parse().expect("valid");
        let name = build(&id, "quarterly-review");
        let parsed = parse(&name).expect("parse");
        assert_eq!(parsed.id, id);
        assert_eq!(parsed.slug, "quarterly-review");
    }

    #[test]
    fn parse_keeps_drifted_slug_verbatim() {
        // A slug that no longer matches any title is still parsed as-is; only
        // the identity is authoritative.
        let parsed = parse(&format!("{ULID}-Stale_Slug.md")).expect("parse");
        assert_eq!(parsed.slug, "Stale_Slug");
    }

    #[test]
    fn parse_rejects_non_markdown() {
        assert!(matches!(
            parse(&format!("{ULID}-note.txt")),
            Err(FilenameError::NotMarkdown(_))
        ));
    }

    #[test]
    fn parse_rejects_too_short() {
        assert!(matches!(parse("short.md"), Err(FilenameError::TooShort(_))));
    }

    #[test]
    fn parse_rejects_missing_separator() {
        // 26 valid ULID chars immediately followed by slug text, no `-`.
        let name = format!("{ULID}xslug.md");
        assert!(matches!(
            parse(&name),
            Err(FilenameError::MissingSeparator(_))
        ));
    }

    #[test]
    fn parse_rejects_bad_ulid() {
        // `I` is outside the Crockford alphabet, so the 26-char prefix is not a
        // ULID.
        let name = "0IARZ3NDEKTSV4RRFFQ69G5FAV-note.md";
        assert!(matches!(parse(name), Err(FilenameError::Id { .. })));
    }

    // A file created outside ntropy can put a multibyte character across the
    // 26th byte, where the identity ends. Such a prefix holds a non-ASCII byte
    // and so can never be a ULID; it must be reported, not split.

    #[test]
    fn parse_rejects_a_two_byte_char_straddling_the_ulid_width() {
        // `é` occupies bytes 25..27.
        let name = format!("{}é-xx.md", "a".repeat(25));
        assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
    }

    #[test]
    fn parse_rejects_a_three_byte_char_straddling_the_ulid_width() {
        // `€` occupies bytes 25..28.
        let name = format!("{}€-xx.md", "a".repeat(25));
        assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
    }

    #[test]
    fn parse_rejects_a_four_byte_char_straddling_the_ulid_width() {
        // `😀` occupies bytes 24..28.
        let name = format!("{}😀-xx.md", "a".repeat(24));
        assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
    }

    #[test]
    fn parse_rejects_a_boundary_aligned_multibyte_prefix() {
        // Thirteen `é` fill exactly 26 bytes, so the split lands on a boundary
        // and the prefix fails as a ULID.
        let name = format!("{}-x.md", "é".repeat(13));
        assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
    }

    #[test]
    fn has_md_extension_accepts_only_lowercase_md() {
        assert!(has_md_extension(Path::new("templates/meeting.md")));
        assert!(!has_md_extension(Path::new("templates/Meeting.MD")));
        assert!(!has_md_extension(Path::new("templates/Meeting.Md")));
        assert!(!has_md_extension(Path::new("notes.markdown")));
        assert!(!has_md_extension(Path::new("README")));
        // A dotfile's name is its stem, not an extension.
        assert!(!has_md_extension(Path::new(".md")));
    }

    #[test]
    fn parse_accepts_a_multibyte_slug() {
        let parsed = parse(&format!("{ULID}-über.md")).expect("parse");
        assert_eq!(parsed.slug, "über");
    }
}
