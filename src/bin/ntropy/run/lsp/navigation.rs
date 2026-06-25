// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Navigation: go-to-definition, document links and workspace symbols
//! (ADR 0028, ADR 0029).
//!
//! Definition and document links share one notion of a link's active span (the
//! whole `[..](..)`), reusing the library's link extraction. Workspace symbols
//! offer every note by title, fuzzy-ranked with `nucleo` (ADR 0027), as a
//! command-palette jump across the vault.

use lsp_types::{
    DocumentLink, GotoDefinitionResponse, Location, OneOf, Position, Range, SymbolKind,
    WorkspaceSymbol,
};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

use ntropy::link;
use ntropy::note::frontmatter;

use super::cache::CacheEntry;
use super::offset::{self, Encoding};
use super::uri;

/// A zero-width range at the start of a file, used to jump to a note.
fn file_start() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

/// Resolve the link under the cursor to its target note's location.
pub fn definition(
    text: &str,
    offset: usize,
    entries: &[CacheEntry],
) -> Option<GotoDefinitionResponse> {
    let body = frontmatter::split(text).body;
    let body_start = text.len() - body.len();
    if offset < body_start {
        return None;
    }
    let links = link::extract(body);
    let link = link::at_offset(&links, offset - body_start)?;
    let entry = entries.iter().find(|entry| entry.id == link.id)?;
    let location = Location::new(uri::from_path(&entry.path)?, file_start());
    Some(GotoDefinitionResponse::Scalar(location))
}

/// All resolvable links in the document, as clickable targets.
///
/// Dangling links (whose ULID resolves to no note) are omitted so no broken
/// target is offered; links in code are never extracted.
pub fn document_links(text: &str, encoding: Encoding, entries: &[CacheEntry]) -> Vec<DocumentLink> {
    let body = frontmatter::split(text).body;
    let body_start = text.len() - body.len();
    link::extract(body)
        .into_iter()
        .filter_map(|link| {
            let entry = entries.iter().find(|entry| entry.id == link.id)?;
            let target = uri::from_path(&entry.path)?;
            let range = Range::new(
                offset::offset_to_position(text, body_start + link.range.start, encoding),
                offset::offset_to_position(text, body_start + link.range.end, encoding),
            );
            Some(DocumentLink {
                range,
                target: Some(target),
                tooltip: None,
                data: None,
            })
        })
        .collect()
}

/// Every note as a workspace symbol, fuzzy-ranked by title for a non-empty
/// query and newest-first (cache order) for an empty one.
pub fn workspace_symbols(query: &str, entries: &[&CacheEntry]) -> Vec<WorkspaceSymbol> {
    ranked(query, entries)
        .into_iter()
        .filter_map(symbol)
        .collect()
}

/// Build a workspace symbol for a note, or `None` if its path has no URI.
fn symbol(entry: &CacheEntry) -> Option<WorkspaceSymbol> {
    let location = Location::new(uri::from_path(&entry.path)?, file_start());
    Some(WorkspaceSymbol {
        name: entry.title.clone(),
        kind: SymbolKind::FILE,
        tags: None,
        container_name: None,
        location: OneOf::Left(location),
        data: None,
    })
}

/// Rank entries by title against the query, keeping cache order for an empty one.
fn ranked<'e>(query: &str, entries: &[&'e CacheEntry]) -> Vec<&'e CacheEntry> {
    if query.is_empty() {
        return entries.to_vec();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut scratch = Vec::new();
    let mut scored: Vec<(u32, usize, &CacheEntry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let haystack = Utf32String::from(entry.title.as_str());
            pattern
                .indices(haystack.slice(..), &mut matcher, &mut scratch)
                .map(|score| {
                    scratch.clear();
                    (score, index, *entry)
                })
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntropy::id::Id;
    use std::path::PathBuf;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn entry(ulid: &str, slug: &str, title: &str) -> CacheEntry {
        CacheEntry {
            id: ulid.parse::<Id>().expect("ulid"),
            title: title.to_owned(),
            tags: Vec::new(),
            path: PathBuf::from(format!("/v/all-notes/{ulid}-{slug}.md")),
        }
    }

    fn entries() -> Vec<CacheEntry> {
        vec![
            entry(ULID_A, "quarterly", "Quarterly Review"),
            entry(ULID_B, "rust", "Rust Notes"),
        ]
    }

    fn target(response: &GotoDefinitionResponse) -> &str {
        match response {
            GotoDefinitionResponse::Scalar(location) => location.uri.as_str(),
            other => panic!("expected a scalar location, got {other:?}"),
        }
    }

    #[test]
    fn definition_jumps_to_the_linked_note() {
        let text = format!("see [Quarterly]({ULID_A}-quarterly.md) here");
        let offset = text.find("Quarterly]").expect("inside the link");
        let response = definition(&text, offset, &entries()).expect("definition");
        assert!(target(&response).ends_with(&format!("{ULID_A}-quarterly.md")));
    }

    #[test]
    fn definition_on_a_dangling_link_is_none() {
        let other = "01CX5ZZKBKACTAV9WEVGEMMVRZ";
        let text = format!("[gone]({other}-x.md)");
        let offset = text.find("gone").unwrap();
        assert!(definition(&text, offset, &entries()).is_none());
    }

    #[test]
    fn definition_off_any_link_is_none() {
        let text = format!("prose [Quarterly]({ULID_A}-quarterly.md)");
        assert!(definition(&text, 0, &entries()).is_none());
    }

    #[test]
    fn definition_in_frontmatter_is_none() {
        let text = format!("---\nlink: [x]({ULID_A}-quarterly.md)\n---\nbody\n");
        let offset = text.find("x]").unwrap();
        assert!(definition(&text, offset, &entries()).is_none());
    }

    #[test]
    fn document_links_resolve_and_omit_dangling() {
        let other = "01CX5ZZKBKACTAV9WEVGEMMVRZ";
        let text =
            format!("[a]({ULID_A}-quarterly.md) and [b]({other}-x.md) and `[c]({ULID_B}-rust.md)`");
        let links = document_links(&text, Encoding::Utf8, &entries());
        // Only the first link resolves; the dangling one and the in-code one drop.
        assert_eq!(links.len(), 1);
        assert!(
            links[0]
                .target
                .as_ref()
                .unwrap()
                .as_str()
                .ends_with(&format!("{ULID_A}-quarterly.md"))
        );
    }

    #[test]
    fn document_link_range_is_document_relative() {
        let text = format!("xy [a]({ULID_A}-quarterly.md)");
        let links = document_links(&text, Encoding::Utf8, &entries());
        assert_eq!(links[0].range.start, Position::new(0, 3));
    }

    #[test]
    fn document_links_empty_body_is_empty() {
        assert!(document_links("no links here", Encoding::Utf8, &entries()).is_empty());
    }

    #[test]
    fn workspace_symbols_empty_query_lists_all() {
        let entries = entries();
        let refs: Vec<&CacheEntry> = entries.iter().collect();
        let symbols = workspace_symbols("", &refs);
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn workspace_symbols_filters_by_title() {
        let entries = entries();
        let refs: Vec<&CacheEntry> = entries.iter().collect();
        let symbols = workspace_symbols("rust", &refs);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Rust Notes");
        assert_eq!(symbols[0].kind, SymbolKind::FILE);
    }

    #[test]
    fn workspace_symbols_no_match_is_empty() {
        let entries = entries();
        let refs: Vec<&CacheEntry> = entries.iter().collect();
        assert!(workspace_symbols("zzzz", &refs).is_empty());
    }
}
