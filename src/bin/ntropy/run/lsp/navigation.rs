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

use std::path::{Path, PathBuf};

use ntropy::id::Id;
use ntropy::link;
use ntropy::note::frontmatter;

use super::cache::CacheEntry;
use super::fuzzy;
use super::offset::{self, Encoding};
use super::uri;

/// A zero-width range at the start of a file, used to jump to a note.
fn file_start() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

/// Resolve the link under the cursor to its target note's location.
pub fn definition(text: &str, offset: usize, entries: &[CacheEntry]) -> Option<(Id, PathBuf)> {
    let body = frontmatter::split(text).body;
    let body_start = text.len() - body.len();
    if offset < body_start {
        return None;
    }
    let links = link::extract(body);
    let link = link::at_offset(&links, offset - body_start)?;
    let entry = entries.iter().find(|entry| entry.id == link.id)?;
    Some((entry.id, entry.path.clone()))
}

/// Build the goto-definition response for an already-resolved target.
///
/// Split from the lookup so the caller can decide *which file* to open: in an
/// encrypted vault that is a decrypted copy, not the ciphertext the entry
/// names (ADR 0041).
pub fn definition_response(path: &Path) -> Option<GotoDefinitionResponse> {
    let location = Location::new(uri::from_path(path)?, file_start());
    Some(GotoDefinitionResponse::Scalar(location))
}

/// All resolvable links in the document, as clickable targets.
///
/// Dangling links (whose ULID resolves to no note) are omitted so no broken
/// target is offered; links in code are never extracted.
pub fn document_links(
    text: &str,
    encoding: Encoding,
    entries: &[CacheEntry],
) -> Vec<(Range, Id, PathBuf)> {
    let body = frontmatter::split(text).body;
    let body_start = text.len() - body.len();
    link::extract(body)
        .into_iter()
        .filter_map(|link| {
            let entry = entries.iter().find(|entry| entry.id == link.id)?;
            let range = Range::new(
                offset::offset_to_position(text, body_start + link.range.start, encoding),
                offset::offset_to_position(text, body_start + link.range.end, encoding),
            );
            Some((range, entry.id, entry.path.clone()))
        })
        .collect()
}

/// Turn a resolved link target into a clickable document link.
pub fn document_link(range: Range, path: &Path) -> Option<DocumentLink> {
    Some(DocumentLink {
        range,
        target: Some(uri::from_path(path)?),
        tooltip: None,
        data: None,
    })
}

/// Every note as a workspace symbol, fuzzy-ranked by title for a non-empty
/// query and newest-first (cache order) for an empty one.
pub fn workspace_symbols(query: &str, entries: &[&CacheEntry]) -> Vec<(Id, PathBuf, String)> {
    fuzzy::rank(query, entries, |entry| entry.title.clone())
        .into_iter()
        .map(|entry| (entry.id, entry.path.clone(), entry.title.clone()))
        .collect()
}

/// Build a workspace symbol for a note, or `None` if its path has no URI.
pub fn symbol(title: &str, path: &Path) -> Option<WorkspaceSymbol> {
    let location = Location::new(uri::from_path(path)?, file_start());
    Some(WorkspaceSymbol {
        name: title.to_owned(),
        kind: SymbolKind::FILE,
        tags: None,
        container_name: None,
        location: OneOf::Left(location),
        data: None,
    })
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
            link_target: format!("{ulid}-{slug}.md"),
        }
    }

    fn entries() -> Vec<CacheEntry> {
        vec![
            entry(ULID_A, "quarterly", "Quarterly Review"),
            entry(ULID_B, "rust", "Rust Notes"),
        ]
    }

    fn target(response: &GotoDefinitionResponse) -> String {
        match response {
            GotoDefinitionResponse::Scalar(location) => location.uri.as_str().to_owned(),
            other => panic!("expected a scalar location, got {other:?}"),
        }
    }

    #[test]
    fn definition_jumps_to_the_linked_note() {
        let text = format!("see [Quarterly]({ULID_A}-quarterly.md) here");
        let offset = text.find("Quarterly]").expect("inside the link");
        let (id, path) = definition(&text, offset, &entries()).expect("definition");
        assert_eq!(id.to_string(), ULID_A);
        assert!(path.ends_with(format!("{ULID_A}-quarterly.md")));

        // The response is built from whatever path the caller decided to open,
        // which in an encrypted vault is a decrypted copy rather than the note.
        let response = definition_response(&path).expect("response");
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
        assert!(links[0].2.ends_with(format!("{ULID_A}-quarterly.md")));

        let link = document_link(links[0].0, &links[0].2).expect("link");
        assert!(
            link.target
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
        assert_eq!(links[0].0.start, Position::new(0, 3));
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
        let ranked = workspace_symbols("rust", &refs);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].2, "Rust Notes");

        let symbol = symbol(&ranked[0].2, &ranked[0].1).expect("symbol");
        assert_eq!(symbol.name, "Rust Notes");
        assert_eq!(symbol.kind, SymbolKind::FILE);
    }

    #[test]
    fn workspace_symbols_no_match_is_empty() {
        let entries = entries();
        let refs: Vec<&CacheEntry> = entries.iter().collect();
        assert!(workspace_symbols("zzzz", &refs).is_empty());
    }
}
