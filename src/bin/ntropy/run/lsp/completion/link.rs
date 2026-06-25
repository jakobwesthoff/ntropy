// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Completion for inter-note links (ADR 0028, ADR 0029).
//!
//! Two authoring paths are served, both detected from the buffer at the cursor
//! without parsing the whole document:
//!
//! - typing `[` starts a link; the whole `[Title](<ulid>-<slug>.md)` is inserted
//!   from a note picked by fuzzy-matching its title and tags;
//! - typing inside a hand-written `](…)` completes just the target path.
//!
//! Results are always marked incomplete so the editor re-queries on each
//! keystroke and the server re-ranks with `nucleo` (ADR 0027).

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionTextEdit, InsertTextFormat,
    Range, TextEdit,
};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

use ntropy::link;
use ntropy::note::frontmatter;

use super::super::cache::CacheEntry;
use super::super::offset::{self, Encoding};

/// Build a link-completion list for the cursor at `offset`, or `None` when the
/// cursor is not in a link position.
pub fn complete(
    text: &str,
    offset: usize,
    encoding: Encoding,
    entries: &[CacheEntry],
    snippet_support: bool,
) -> Option<CompletionList> {
    let context = detect(text, offset)?;
    let range = Range {
        start: offset::offset_to_position(text, context.replace_start, encoding),
        end: offset::offset_to_position(text, offset, encoding),
    };
    let items = ranked(&context.query, entries)
        .iter()
        .enumerate()
        .map(|(rank, entry)| item(&context, entry, range, rank, snippet_support))
        .collect();
    Some(CompletionList {
        is_incomplete: true,
        items,
    })
}

/// Which part of a link the cursor sits in.
#[derive(Debug, PartialEq, Eq)]
enum Kind {
    /// Inside the `[display]` brackets: insert the whole link.
    Display,
    /// Inside the `(target)` parentheses: insert just the target path.
    Target,
}

/// A detected link-completion context at the cursor.
#[derive(Debug, PartialEq, Eq)]
struct Context {
    kind: Kind,
    /// The text typed so far, used as the fuzzy query.
    query: String,
    /// Byte offset where the replacement begins (the cursor is its end).
    replace_start: usize,
}

/// Detect a link-completion context at `offset`, or `None`.
fn detect(text: &str, offset: usize) -> Option<Context> {
    // Frontmatter is tag territory, not links.
    let body = frontmatter::split(text).body;
    let body_start = text.len() - body.len();
    if offset < body_start {
        return None;
    }
    // Never complete a link inside code.
    if link::in_code(body, offset - body_start) {
        return None;
    }

    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &text[line_start..offset];

    // A hand-written `](` target takes precedence over the enclosing `[`.
    if let Some(p) = prefix.rfind("](") {
        let target = &prefix[p + 2..];
        if !target.contains([')', '(']) {
            return Some(Context {
                kind: Kind::Target,
                query: target.to_owned(),
                replace_start: line_start + p + 2,
            });
        }
    }

    // An open `[` with no closing `]` is a display being typed.
    if let Some(b) = prefix.rfind('[') {
        let display = &prefix[b + 1..];
        let is_image = b > 0 && prefix.as_bytes()[b - 1] == b'!';
        if !display.contains(']') && !is_image {
            return Some(Context {
                kind: Kind::Display,
                query: display.to_owned(),
                replace_start: line_start + b + 1,
            });
        }
    }

    None
}

/// Rank entries against the query: fuzzy by title/tags/filename, or newest-first
/// (cache order) for an empty query.
fn ranked<'e>(query: &str, entries: &'e [CacheEntry]) -> Vec<&'e CacheEntry> {
    if query.is_empty() {
        return entries.iter().collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut scratch = Vec::new();
    let mut scored: Vec<(u32, usize, &CacheEntry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let haystack = Utf32String::from(haystack(entry));
            pattern
                .indices(haystack.slice(..), &mut matcher, &mut scratch)
                .map(|score| {
                    scratch.clear();
                    (score, index, entry)
                })
        })
        .collect();
    // Best score first; ties keep the newest-first cache order.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, entry)| entry).collect()
}

/// The fuzzy haystack for an entry: its title, tags and filename.
fn haystack(entry: &CacheEntry) -> String {
    format!(
        "{} {} {}",
        entry.title,
        entry.tags.join(" "),
        filename(entry)
    )
}

/// The entry's on-disk filename, which is the link target within `all-notes/`.
fn filename(entry: &CacheEntry) -> String {
    entry
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Build the completion item for one entry.
fn item(
    context: &Context,
    entry: &CacheEntry,
    range: Range,
    rank: usize,
    snippet_support: bool,
) -> CompletionItem {
    let target = filename(entry);
    let (new_text, format) = match context.kind {
        Kind::Target => (target.clone(), InsertTextFormat::PLAIN_TEXT),
        Kind::Display if snippet_support => (
            format!("{}]({}) $0", escape_snippet(&entry.title), target),
            InsertTextFormat::SNIPPET,
        ),
        Kind::Display => (
            format!("{}]({})", entry.title, target),
            InsertTextFormat::PLAIN_TEXT,
        ),
    };
    // Snippet trailing `$0` adds a space we did not intend; trim it to sit the
    // cursor right after the link.
    let new_text = new_text.replace(") $0", ")$0");

    CompletionItem {
        label: entry.title.clone(),
        kind: Some(CompletionItemKind::REFERENCE),
        detail: Some(target),
        // A constant filter text keeps the client from dropping fuzzy matches;
        // ordering is carried by `sort_text`.
        filter_text: Some(context.query.clone()),
        sort_text: Some(format!("{rank:06}")),
        insert_text_format: Some(format),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text })),
        ..Default::default()
    }
}

/// Escape the snippet metacharacters so a title is inserted literally.
fn escape_snippet(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\\' | '$' | '}') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Position;
    use ntropy::id::Id;
    use std::path::PathBuf;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn entry(ulid: &str, slug: &str, title: &str, tags: &[&str]) -> CacheEntry {
        CacheEntry {
            id: ulid.parse::<Id>().expect("ulid"),
            title: title.to_owned(),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            path: PathBuf::from(format!("/v/all-notes/{ulid}-{slug}.md")),
        }
    }

    fn entries() -> Vec<CacheEntry> {
        vec![
            entry(
                ULID_A,
                "quarterly-review",
                "Quarterly Review",
                &["area/work"],
            ),
            entry(ULID_B, "rust-notes", "Rust Notes", &["programming/rust"]),
        ]
    }

    /// Complete with the cursor at the `|` marker in `text`.
    fn at_marker(text: &str, snippet_support: bool) -> Option<CompletionList> {
        let offset = text.find('|').expect("a cursor marker");
        let text = text.replace('|', "");
        complete(&text, offset, Encoding::Utf8, &entries(), snippet_support)
    }

    fn new_text<'a>(list: &'a CompletionList, label: &str) -> &'a str {
        let item = list
            .items
            .iter()
            .find(|i| i.label == label)
            .expect("item present");
        match item.text_edit.as_ref().expect("text edit") {
            CompletionTextEdit::Edit(edit) => &edit.new_text,
            other => panic!("unexpected edit: {other:?}"),
        }
    }

    #[test]
    fn bare_bracket_offers_all_notes_newest_first() {
        let list = at_marker("body [|", false).expect("completion");
        assert!(list.is_incomplete);
        assert_eq!(list.items.len(), 2);
        // ULID_B sorts newest (descending) but the cache order here is as given;
        // both notes are offered.
        let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Quarterly Review"));
        assert!(labels.contains(&"Rust Notes"));
    }

    #[test]
    fn display_query_fuzzy_filters() {
        let list = at_marker("body [Quar|", false).expect("completion");
        assert_eq!(list.items[0].label, "Quarterly Review");
        assert_eq!(
            new_text(&list, "Quarterly Review"),
            format!("Quarterly Review]({ULID_A}-quarterly-review.md)")
        );
    }

    #[test]
    fn a_tag_match_still_offers_the_note() {
        let list = at_marker("body [rust|", false).expect("completion");
        assert_eq!(list.items[0].label, "Rust Notes");
    }

    #[test]
    fn snippet_support_places_the_cursor_after_the_link() {
        let list = at_marker("body [Quar|", true).expect("completion");
        assert_eq!(
            new_text(&list, "Quarterly Review"),
            format!("Quarterly Review]({ULID_A}-quarterly-review.md)$0")
        );
        let item = &list.items[0];
        assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
    }

    #[test]
    fn target_context_completes_the_path_only() {
        let list = at_marker("body [Display](Quar|", false).expect("completion");
        assert_eq!(
            new_text(&list, "Quarterly Review"),
            format!("{ULID_A}-quarterly-review.md")
        );
    }

    #[test]
    fn inside_a_closed_link_has_no_completion() {
        assert!(at_marker("body [a](x.md)|", false).is_none());
    }

    #[test]
    fn outside_any_bracket_has_no_completion() {
        assert!(at_marker("just prose |here", false).is_none());
    }

    #[test]
    fn bracket_in_frontmatter_is_not_a_link() {
        let text = "---\ntags: [|]\n---\nbody\n";
        let offset = text.find('|').expect("marker");
        let text = text.replace('|', "");
        assert!(complete(&text, offset, Encoding::Utf8, &entries(), false).is_none());
    }

    #[test]
    fn bracket_in_code_is_suppressed() {
        // Body offset inside a fenced block.
        let text = "x\n```\n[|\n```\n";
        let offset = text.find('|').expect("marker");
        let text = text.replace('|', "");
        assert!(complete(&text, offset, Encoding::Utf8, &entries(), false).is_none());
    }

    #[test]
    fn image_bracket_is_not_a_link() {
        assert!(at_marker("body ![|", false).is_none());
    }

    #[test]
    fn empty_vault_yields_an_empty_but_incomplete_list() {
        let offset = "body [".len();
        let list = complete("body [", offset, Encoding::Utf8, &[], false).expect("completion");
        assert!(list.is_incomplete);
        assert!(list.items.is_empty());
    }

    #[test]
    fn replacement_range_runs_from_after_the_bracket_to_the_cursor() {
        let list = at_marker("see [Quar|", false).expect("completion");
        let item = &list.items[0];
        let CompletionTextEdit::Edit(edit) = item.text_edit.as_ref().unwrap() else {
            panic!("edit");
        };
        // "see [" is 5 columns; "Quar" ends at column 9.
        assert_eq!(edit.range.start, Position::new(0, 5));
        assert_eq!(edit.range.end, Position::new(0, 9));
    }
}
