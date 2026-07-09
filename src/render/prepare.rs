// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Assembling the shared [`PreparedDocument`] every engine consumes.
//!
//! Preparation is the lossless half of rendering: it copies the note's facts
//! and resolves its links against the vault, but never discards information and
//! never shapes anything for a particular output. The lossy, output-shaped
//! choices (flattening a link to styled text, stripping frontmatter, picking a
//! date format) belong to the engines, so different engines can materialize the
//! same facts differently. That split is the guardrail for what may live here.

use crate::link::{self, NoteIndex};
use crate::note::Note;

use super::{PreparedDocument, ResolvedLink};

/// Build the lossless [`PreparedDocument`] for `note`, resolving its links
/// against `index`.
///
/// The note's own fields are copied verbatim. The one derived value is
/// `created`, the ULID's creation date in the system-local timezone (ADR 0010);
/// deriving it is the sole fallible step, since a corrupt id can carry a
/// timestamp outside the supported year range.
///
/// Each link resolves to the target note's *current* title through `index`, not
/// the link's own display text, so the prepared table always reflects the vault
/// as it stands now. A link whose id is absent from the index is dangling and
/// carries `target_title = None`.
pub fn prepare(note: &Note, index: &NoteIndex<'_>) -> crate::error::Result<PreparedDocument> {
    let links = link::extract(&note.body)
        .into_iter()
        .map(|link| ResolvedLink {
            range: link.range,
            display: link.display.to_string(),
            id: link.id,
            target_title: index.get(&link.id).map(|target| target.title.clone()),
        })
        .collect();

    Ok(PreparedDocument {
        id: note.id,
        path: note.path.clone(),
        title: note.title.clone(),
        tags: note.tags.clone(),
        created: note.created_date()?,
        frontmatter: note.frontmatter.clone(),
        body: note.body.clone(),
        links,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    // Two fixed identities give deterministic paths and ULID-derived dates in
    // snapshots; a source note and a link target.
    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// Parse a note straight from synthetic inputs, matching the note and link
    /// module tests. A fixed path keeps `PreparedDocument::path` deterministic
    /// without a temporary directory leaking into snapshots.
    fn note(id: &str, slug: &str, content: &str) -> Note {
        Note::parse(
            PathBuf::from(format!("/vault/all-notes/{id}-{slug}.md")),
            content,
            None,
        )
        .expect("note parses")
    }

    /// Snapshot a prepared document with its `created` date redacted. The date
    /// is rendered in the host's local timezone, so it varies by machine; a
    /// dedicated test pins it against `Note::created_date` instead.
    macro_rules! assert_prepared_snapshot {
        ($doc:expr, @$snapshot:literal) => {
            insta::with_settings!(
                {filters => vec![(r#"created: "\d{4}-\d{2}-\d{2}""#, r#"created: "[DATE]""#)]},
                { insta::assert_debug_snapshot!($doc, @$snapshot); }
            );
        };
    }

    #[test]
    fn standard_note_is_copied_losslessly_with_custom_fields() {
        // A `status` field beyond the recognized ones proves the full mapping is
        // carried, not just the lifted title and tags.
        let source = note(
            ULID_A,
            "quarterly-review",
            "---\ntitle: Quarterly Review\ntags: [area/work, programming/rust]\nstatus: draft\n---\nThe body text.\n",
        );
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_prepared_snapshot!(doc, @r#"
        PreparedDocument {
            id: Id(
                Ulid(
                    1777027686520646174104517696511196507,
                ),
            ),
            path: "/vault/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-quarterly-review.md",
            title: "Quarterly Review",
            tags: [
                "area/work",
                "programming/rust",
            ],
            created: "[DATE]",
            frontmatter: Mapping {
                "title": String("Quarterly Review"),
                "tags": Sequence [
                    String("area/work"),
                    String("programming/rust"),
                ],
                "status": String("draft"),
            },
            body: "The body text.\n",
            links: [],
        }
        "#);
    }

    #[test]
    fn empty_body_prepares_without_links() {
        let source = note(ULID_A, "empty", "---\ntitle: Empty\n---\n");
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.body, "");
        assert!(doc.links.is_empty());
    }

    #[test]
    fn body_without_links_has_an_empty_link_table() {
        let source = note(
            ULID_A,
            "prose",
            "---\ntitle: Prose\n---\nJust prose, no links at all.\n",
        );
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.body, "Just prose, no links at all.\n");
        assert!(doc.links.is_empty());
    }

    #[test]
    fn resolved_link_carries_the_targets_current_title_not_the_display_text() {
        // The link's display text is stale; resolution must yield the target's
        // present title so the prepared table reflects the vault as it stands.
        let target = note(ULID_B, "renamed", "---\ntitle: The Current Title\n---\nx\n");
        let source = note(
            ULID_A,
            "source",
            &format!("---\ntitle: Source\n---\nsee [Old Display Text]({ULID_B}-renamed.md) here\n"),
        );
        let notes = vec![source.clone(), target];
        let index = link::index(&notes);
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.links.len(), 1);
        let link = &doc.links[0];
        assert_eq!(link.display, "Old Display Text");
        assert_eq!(link.target_title.as_deref(), Some("The Current Title"));
    }

    #[test]
    fn dangling_link_has_no_target_title() {
        let source = note(
            ULID_A,
            "source",
            &format!("---\ntitle: Source\n---\nsee [Gone]({ULID_B}-gone.md) here\n"),
        );
        // The target id is absent from the index, so the link is dangling.
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].target_title, None);
    }

    #[test]
    fn links_in_code_and_image_links_are_excluded() {
        // Extraction already skips fenced code, inline code and images; this
        // pins that the prepared table honors those exclusions from the
        // consumer's side. Only the one prose link survives.
        let source = note(
            ULID_A,
            "mixed",
            &format!(
                "---\ntitle: Mixed\n---\n\
                 prose [Real]({ULID_B}-real.md)\n\
                 ```\n[Fenced]({ULID_B}-fenced.md)\n```\n\
                 inline `[Inline]({ULID_B}-inline.md)` code\n\
                 ![Image]({ULID_B}-image.md)\n",
            ),
        );
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.links.len(), 1);
        assert_eq!(doc.links[0].display, "Real");
    }

    #[test]
    fn unicode_title_is_preserved_and_tags_are_copied_losslessly() {
        // The title is stored verbatim, so its unicode survives untouched. Tags
        // are already normalized by note parsing (ADR 0023 slugs tag segments to
        // ASCII); preparation copies whatever the note holds, so the check is
        // that `doc.tags` equals the note's tags exactly, not that they carry
        // unicode of their own.
        let source = note(
            ULID_A,
            "unicode",
            "---\ntitle: Über Größe – café 日本語\ntags: [Área/Work, Life/Café]\n---\nbody\n",
        );
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.title, "Über Größe – café 日本語");
        assert_eq!(doc.tags, source.tags);
    }

    #[test]
    fn empty_tags_prepare_as_an_empty_vector() {
        let source = note(ULID_A, "untagged", "---\ntitle: Untagged\n---\nbody\n");
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert!(doc.tags.is_empty());
    }

    #[test]
    fn two_links_to_one_target_keep_distinct_byte_ranges() {
        // Both links point at the same note, so both resolve to its title, yet
        // each must keep its own span so an engine can substitute them
        // independently.
        let target = note(ULID_B, "target", "---\ntitle: Shared Target\n---\nx\n");
        let body = format!("[first]({ULID_B}-target.md) then [second]({ULID_B}-target.md)\n");
        let source = note(
            ULID_A,
            "source",
            &format!("---\ntitle: Source\n---\n{body}"),
        );
        let notes = vec![source.clone(), target];
        let index = link::index(&notes);
        let doc = prepare(&source, &index).expect("prepare");

        assert_eq!(doc.links.len(), 2);
        let first = &doc.links[0];
        let second = &doc.links[1];
        // Ranges are relative to the body, and the two spans are disjoint.
        assert_eq!(
            &doc.body[first.range.clone()],
            format!("[first]({ULID_B}-target.md)")
        );
        assert_eq!(
            &doc.body[second.range.clone()],
            format!("[second]({ULID_B}-target.md)")
        );
        assert!(first.range.end <= second.range.start);
        assert_eq!(first.target_title.as_deref(), Some("Shared Target"));
        assert_eq!(second.target_title.as_deref(), Some("Shared Target"));
    }

    #[test]
    fn created_matches_the_notes_derived_date() {
        // The snapshots redact the local-timezone date; this pins that `created`
        // is exactly what the note derives, whatever the host zone renders.
        let source = note(ULID_A, "dated", "---\ntitle: Dated\n---\nbody\n");
        let index = link::index(std::slice::from_ref(&source));
        let doc = prepare(&source, &index).expect("prepare");
        assert_eq!(doc.created, source.created_date().expect("date"));
    }
}
