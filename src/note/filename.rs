// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Building and parsing the canonical `<ulid>-<slug>.md` filename (ADR 0004).
//!
//! The ULID is fixed-width (26 chars) and leads the name, so the split is by
//! position: the first 26 characters are the identity, a single `-` separates
//! it from the slug, and the slug runs to the `.md` extension. Identity is read
//! from the filename and never stored in frontmatter, so this parse is the only
//! place a note's id is recovered from disk.

use crate::id::{Id, ULID_LEN};
use crate::text::slug;

/// The canonical note file extension.
const MD_EXT: &str = ".md";

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

/// Why a top-level `.md` filename is not a canonical note name.
#[derive(Debug, thiserror::Error)]
pub enum FilenameError {
    #[error("`{0}` does not have a .md extension")]
    NotMarkdown(String),
    #[error("`{0}` is too short to be a <ulid>-<slug>.md name")]
    TooShort(String),
    #[error("`{0}` is missing the `-` separator after the ULID")]
    MissingSeparator(String),
    #[error("`{name}` does not start with a valid ULID")]
    Id {
        name: String,
        #[source]
        source: crate::id::IdError,
    },
}

/// Build the canonical filename for an identity and slug.
pub fn build(id: &Id, slug: &str) -> String {
    format!("{id}-{slug}{MD_EXT}")
}

/// Build the canonical filename, deriving the slug from a title.
pub fn build_from_title(id: &Id, title: &str) -> String {
    build(id, &slug::slugify(title))
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

    let (id_part, rest) = stem.split_at(ULID_LEN);
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
}
