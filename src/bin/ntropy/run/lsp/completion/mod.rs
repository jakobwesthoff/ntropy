// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Completion dispatch (ADR 0029).
//!
//! A completion request is first offered to link completion; if the cursor is
//! not in a link position, frontmatter tag completion is tried instead. At most
//! one applies at a cursor, so the first that matches wins.

mod link;
mod tag;

use std::collections::HashSet;

use lsp_types::CompletionList;

use super::cache::CacheEntry;
use super::offset::Encoding;

/// Build a completion list for the cursor, trying links then frontmatter tags.
pub fn complete(
    text: &str,
    offset: usize,
    encoding: Encoding,
    entries: &[CacheEntry],
    snippet_support: bool,
) -> Option<CompletionList> {
    link::complete(text, offset, encoding, entries, snippet_support)
        .or_else(|| tag::complete(text, offset, encoding, &unique_tags(entries)))
}

/// The distinct tags across all cached notes, sorted for stable suggestions.
fn unique_tags(entries: &[CacheEntry]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut tags: Vec<String> = entries
        .iter()
        .flat_map(|entry| entry.tags.iter())
        .filter(|tag| seen.insert((*tag).clone()))
        .cloned()
        .collect();
    tags.sort();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntropy::id::Id;
    use std::path::PathBuf;

    fn entry(ulid: &str, tags: &[&str]) -> CacheEntry {
        CacheEntry {
            id: ulid.parse::<Id>().expect("ulid"),
            title: "T".to_owned(),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            path: PathBuf::from(format!("/v/all-notes/{ulid}-t.md")),
        }
    }

    #[test]
    fn unique_tags_dedupes_and_sorts() {
        let entries = vec![
            entry(
                "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                &["programming/rust", "area/work"],
            ),
            entry(
                "01BRZ3NDEKTSV4RRFFQ69G5FAV",
                &["area/work", "programming/cli"],
            ),
        ];
        assert_eq!(
            unique_tags(&entries),
            vec!["area/work", "programming/cli", "programming/rust"]
        );
    }

    #[test]
    fn dispatch_prefers_link_then_tag_then_nothing() {
        let entries = vec![entry("01ARZ3NDEKTSV4RRFFQ69G5FAV", &["area/work"])];

        // A `[` in the body is a link context.
        let body = "text [";
        let link = complete(body, body.len(), Encoding::Utf8, &entries, false);
        assert!(link.is_some());

        // A tag value inside a closed frontmatter is a tag context.
        let fm = "---\ntags: [\n---\n";
        let offset = "---\ntags: [".len();
        let tag = complete(fm, offset, Encoding::Utf8, &entries, false);
        assert!(tag.is_some());

        // Plain prose is neither.
        let prose = "just text here";
        assert!(complete(prose, prose.len(), Encoding::Utf8, &entries, false).is_none());
    }
}
