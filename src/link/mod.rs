// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Inter-note links: extraction and resolution (ADR 0028).
//!
//! A note-to-note link is a standard Markdown inline link whose target is the
//! current filename of the target note, `[display](<ulid>-<slug>.md)`. Identity
//! is carried by the leading 26-character ULID of the target; the slug and the
//! `.md` make it a real, clickable path. This module finds such links in a note
//! body and resolves a link's ULID to a scanned note.
//!
//! The surface is intentionally infallible: extraction yields the links it can
//! validate and resolution yields an `Option`, so callers (the language server
//! and `reconcile`) never thread a link-specific error type through the crate.

mod code;

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use crate::id::{Id, ULID_LEN};
use crate::note::Note;

/// A validated inter-note link found in a note body.
///
/// All byte ranges are relative to the body the link was extracted from; the
/// language server adds the body's offset within the document before reporting
/// them to a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link<'a> {
    /// The whole `[display](target)` span: the link's active region for
    /// go-to-definition and document links.
    pub range: Range<usize>,
    /// The target span inside the parentheses, used to rewrite a stale slug.
    pub target_range: Range<usize>,
    /// The display text between the brackets.
    pub display: &'a str,
    /// The raw target between the parentheses.
    pub target: &'a str,
    /// The note identity parsed from the target's 26-character prefix.
    pub id: Id,
}

/// A candidate Markdown inline link. Acceptance as an ntropy link is decided by
/// [`parse_target`] (a valid ULID prefix and a `.md` suffix) and by the image
/// and code-region checks in [`extract`]; this only finds the shape.
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)\s]+)\)").expect("the link pattern is valid"));

/// Extract every ntropy link from `body`, in document order.
///
/// Image links (`![..](..)`) and links inside fenced or inline code are
/// skipped, as is any link whose target is not `<26-char ULID>[-<slug>].md`.
pub fn extract(body: &str) -> Vec<Link<'_>> {
    let masked = code::masked_ranges(body);
    let mut links = Vec::new();
    for captures in LINK_RE.captures_iter(body) {
        let whole = captures.get(0).expect("the whole match always exists");
        // A preceding `!` makes this an image, not a note link.
        if whole.start() > 0 && body.as_bytes()[whole.start() - 1] == b'!' {
            continue;
        }
        if code::is_masked(&masked, whole.start()) {
            continue;
        }
        let display = captures.get(1).expect("the display group always exists");
        let target = captures.get(2).expect("the target group always exists");
        let Some(id) = parse_target(target.as_str()) else {
            continue;
        };
        links.push(Link {
            range: whole.start()..whole.end(),
            target_range: target.start()..target.end(),
            display: display.as_str(),
            target: target.as_str(),
            id,
        });
    }
    links
}

/// Resolve a link identity to a scanned note, or `None` when it is dangling.
pub fn resolve(id: Id, notes: &[Note]) -> Option<&Note> {
    notes.iter().find(|note| note.id == id)
}

/// The link covering `offset`, if any. Used to act on the link under a cursor.
pub fn at_offset<'l, 'b>(links: &'l [Link<'b>], offset: usize) -> Option<&'l Link<'b>> {
    links.iter().find(|link| link.range.contains(&offset))
}

/// Parse a link target into a note identity, accepting only
/// `<26-char ULID>` optionally followed by `-<slug>`, ending in `.md`.
fn parse_target(target: &str) -> Option<Id> {
    if !target.is_char_boundary(ULID_LEN) {
        return None;
    }
    let (prefix, rest) = target.split_at(ULID_LEN);
    let valid_rest = rest == ".md" || (rest.starts_with('-') && rest.ends_with(".md"));
    if !valid_rest {
        return None;
    }
    prefix.parse::<Id>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_LOWER: &str = "01arz3ndektsv4rrffq69g5fav";

    fn only(body: &str) -> Link<'_> {
        let links = extract(body);
        assert_eq!(links.len(), 1, "expected exactly one link in {body:?}");
        links.into_iter().next().unwrap()
    }

    fn note(id: &str) -> Note {
        let content = "---\ntitle: T\n---\nbody\n";
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{id}-t.md")),
            content,
            None,
        )
        .expect("note parses")
    }

    #[test]
    fn extracts_a_single_link_with_spans() {
        let body = format!("see [Quarterly]({ULID}-quarterly.md) now");
        let link = only(&body);
        assert_eq!(link.display, "Quarterly");
        assert_eq!(link.target, format!("{ULID}-quarterly.md"));
        assert_eq!(
            &body[link.range.clone()],
            format!("[Quarterly]({ULID}-quarterly.md)")
        );
        assert_eq!(&body[link.target_range.clone()], link.target);
        assert_eq!(link.id, ULID.parse::<Id>().unwrap());
    }

    #[test]
    fn extracts_multiple_links_on_one_line() {
        let body = format!("[a]({ULID}-a.md) and [b]({ULID}-b.md)");
        assert_eq!(extract(&body).len(), 2);
    }

    #[test]
    fn extracts_links_across_lines() {
        let body = format!("[a]({ULID}-a.md)\ntext\n[b]({ULID}-b.md)");
        assert_eq!(extract(&body).len(), 2);
    }

    #[test]
    fn empty_display_is_allowed() {
        let body = format!("[]({ULID}-x.md)");
        assert_eq!(only(&body).display, "");
    }

    #[test]
    fn umlaut_display_is_preserved() {
        let body = format!("[Über Größe]({ULID}-ueber-groesse.md)");
        assert_eq!(only(&body).display, "Über Größe");
    }

    #[test]
    fn lowercase_and_mixed_case_ulid_accepted() {
        let lower = format!("[a]({ULID_LOWER}-x.md)");
        assert_eq!(only(&lower).id, ULID.parse::<Id>().unwrap());
        let mixed = format!("[a]({}-x.md)", "01Arz3ndektsv4rrffq69g5fav");
        assert_eq!(only(&mixed).id, ULID.parse::<Id>().unwrap());
    }

    #[test]
    fn target_without_slug_is_accepted() {
        let body = format!("[a]({ULID}.md)");
        assert_eq!(only(&body).id, ULID.parse::<Id>().unwrap());
    }

    #[test]
    fn external_anchor_and_image_links_are_rejected() {
        assert!(extract("[x](https://example.com)").is_empty());
        assert!(extract("[x](#section)").is_empty());
        assert!(extract(&format!("![alt]({ULID}-x.md)")).is_empty());
    }

    #[test]
    fn invalid_ulid_or_missing_md_is_rejected() {
        // Contains `I`, which is outside Crockford base32.
        assert!(extract("[x](01ARZ3NDEKTSV4RRFFQ69G5FAI-x.md)").is_empty());
        // Too short to hold a ULID.
        assert!(extract("[x](01ARZ3-x.md)").is_empty());
        // No `.md` suffix.
        assert!(extract(&format!("[x]({ULID}-x)")).is_empty());
        // Junk between the ULID and `.md` without a separating hyphen.
        assert!(extract(&format!("[x]({ULID}extra.md)")).is_empty());
    }

    #[test]
    fn links_inside_code_are_skipped() {
        let fenced = format!("```\n[a]({ULID}-a.md)\n```");
        assert!(extract(&fenced).is_empty());
        let inline = format!("`[a]({ULID}-a.md)`");
        assert!(extract(&inline).is_empty());
    }

    #[test]
    fn unbalanced_brackets_do_not_panic_or_match() {
        assert!(extract("[oops (no close").is_empty());
        assert!(extract("](stray").is_empty());
    }

    #[test]
    fn resolve_finds_and_misses() {
        let notes = vec![note(ULID)];
        let id: Id = ULID.parse().unwrap();
        assert_eq!(resolve(id, &notes).map(|n| n.id), Some(id));

        let other: Id = "01BX5ZZKBKACTAV9WEVGEMMVRZ".parse().unwrap();
        assert!(resolve(other, &notes).is_none());
        assert!(resolve(id, &[]).is_none());
    }

    #[test]
    fn lowercase_target_resolves_to_uppercase_note() {
        let notes = vec![note(ULID)];
        let body = format!("[a]({ULID_LOWER}-x.md)");
        let link = only(&body);
        assert!(resolve(link.id, &notes).is_some());
    }

    #[test]
    fn at_offset_finds_the_covering_link() {
        let body = format!("xx [a]({ULID}-a.md) yy");
        let links = extract(&body);
        let inside = body.find("a]").unwrap();
        assert!(at_offset(&links, inside).is_some());
        assert!(at_offset(&links, 0).is_none());
    }
}
