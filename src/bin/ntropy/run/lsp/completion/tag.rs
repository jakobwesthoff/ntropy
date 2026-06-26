// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Completion for frontmatter tags (ADR 0006, ADR 0029).
//!
//! Tags are completed against the vault's existing tag set, hierarchy-aware
//! (`ntropy::text::tag::suggest`). The cursor context is detected without a YAML
//! parser, handling the two hand-authored shapes:
//!
//! - flow: `tags: [a, b|]` (single line only);
//! - block: a `tags:` key followed by `- a` / `- b|` items.

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionTextEdit, InsertTextFormat,
    Range, TextEdit,
};

use ntropy::note::frontmatter;

use super::super::offset::{self, Encoding};

/// Build a tag-completion list for the cursor, or `None` outside a tag value.
pub fn complete(
    text: &str,
    offset: usize,
    encoding: Encoding,
    tags: &[String],
) -> Option<CompletionList> {
    let context = detect(text, offset)?;
    let range = Range {
        start: offset::offset_to_position(text, context.replace_start, encoding),
        end: offset::offset_to_position(text, offset, encoding),
    };
    let items = ntropy::text::tag::suggest(&context.query, tags)
        .into_iter()
        .enumerate()
        .map(|(rank, tag)| item(tag, &context.query, range, rank))
        .collect();
    Some(CompletionList {
        is_incomplete: true,
        items,
    })
}

/// A detected tag-completion context.
#[derive(Debug, PartialEq, Eq)]
struct Context {
    /// The partial tag typed so far.
    query: String,
    /// Byte offset where the replacement begins (the cursor is its end).
    replace_start: usize,
}

/// Detect a frontmatter tag-completion context at `offset`, or `None`.
fn detect(text: &str, offset: usize) -> Option<Context> {
    // Only within a closed frontmatter block, and not in the body.
    let split = frontmatter::split(text);
    split.frontmatter?;
    let body_start = text.len() - split.body.len();
    if offset >= body_start {
        return None;
    }

    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &text[line_start..offset];
    let stripped = prefix.trim_start();

    // Flow form: `tags: [a, b|` on a single line.
    if let Some(after_key) = stripped.strip_prefix("tags:") {
        if let Some(open) = after_key.find('[')
            && !after_key[open + 1..].contains(']')
        {
            // The last `[` or `,` before the cursor opens the current element.
            // A comma inside a value cannot mislead this: commas do not survive
            // tag normalization, so no candidate is ever offered against one.
            let separator = prefix
                .rfind(['[', ','])
                .expect("the flow array has an opening bracket");
            return Some(token_after(prefix, line_start, separator));
        }
        return None;
    }

    // Block form: a `- item` line beneath a `tags:` key.
    if stripped.starts_with('-') && under_tags_key(text, line_start) {
        // The list dash is the first non-whitespace byte of the line, not the
        // last `-` on it: a hyphen inside the value (`work-home`) is routine and
        // `rfind('-')` would split the token mid-value.
        let separator = prefix.len() - stripped.len();
        return Some(token_after(prefix, line_start, separator));
    }

    None
}

/// The token between the separator and the cursor, skipping spaces and one
/// opening quote, with its absolute start offset.
fn token_after(prefix: &str, line_start: usize, separator: usize) -> Context {
    let bytes = prefix.as_bytes();
    let mut start = separator + 1;
    while start < prefix.len() && bytes[start] == b' ' {
        start += 1;
    }
    if start < prefix.len() && bytes[start] == b'"' {
        start += 1;
    }
    Context {
        query: prefix[start..].trim_end_matches('"').to_owned(),
        replace_start: line_start + start,
    }
}

/// Whether the lines above `line_start` are contiguous `- item` lines leading up
/// to a `tags:` key (an intervening blank line or other key breaks the chain).
fn under_tags_key(text: &str, line_start: usize) -> bool {
    if line_start == 0 {
        return false;
    }
    let above = &text[..line_start - 1];
    for line in above.rsplit('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }
        if line.trim_start().starts_with('-') {
            continue;
        }
        return trimmed
            .strip_prefix("tags:")
            .is_some_and(|rest| rest.trim().is_empty());
    }
    false
}

/// Build the completion item inserting `tag` over the typed query.
fn item(tag: String, query: &str, range: Range, rank: usize) -> CompletionItem {
    CompletionItem {
        label: tag.clone(),
        kind: Some(CompletionItemKind::ENUM_MEMBER),
        filter_text: Some(query.to_owned()),
        sort_text: Some(format!("{rank:06}")),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: tag,
        })),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags() -> Vec<String> {
        // Sorted, as the dispatcher supplies the unique tag set.
        ["area/work", "programming/cli", "programming/rust"]
            .iter()
            .map(|t| (*t).to_owned())
            .collect()
    }

    /// Complete at the `|` marker, returning the offered labels.
    fn labels_at(text: &str) -> Option<Vec<String>> {
        let offset = text.find('|').expect("a cursor marker");
        let text = text.replace('|', "");
        complete(&text, offset, Encoding::Utf8, &tags())
            .map(|list| list.items.into_iter().map(|i| i.label).collect())
    }

    #[test]
    fn body_cursor_is_not_a_tag_context() {
        assert!(labels_at("---\ntags: [a]\n---\nbody [|").is_none());
    }

    #[test]
    fn document_without_frontmatter_declines() {
        assert!(labels_at("no frontmatter tags: [|").is_none());
    }

    #[test]
    fn flow_empty_offers_all() {
        assert_eq!(
            labels_at("---\ntags: [|]\n---\n").unwrap(),
            vec!["area/work", "programming/cli", "programming/rust"]
        );
    }

    #[test]
    fn flow_partial_filters() {
        assert_eq!(
            labels_at("---\ntags: [prog|]\n---\n").unwrap(),
            vec!["programming/cli", "programming/rust"]
        );
    }

    #[test]
    fn flow_after_a_comma_uses_the_new_token() {
        assert_eq!(
            labels_at("---\ntags: [area/work, prog|]\n---\n").unwrap(),
            vec!["programming/cli", "programming/rust"]
        );
    }

    #[test]
    fn flow_strips_a_leading_quote() {
        assert_eq!(
            labels_at("---\ntags: [\"area|\"]\n---\n").unwrap(),
            vec!["area/work"]
        );
    }

    #[test]
    fn flow_without_open_bracket_declines() {
        assert!(labels_at("---\ntags:|\n---\n").is_none());
    }

    #[test]
    fn flow_is_single_line_only() {
        // The cursor is on a continuation line, not the `tags:` line.
        assert!(labels_at("---\ntags: [a,\n  prog|\n---\n").is_none());
    }

    #[test]
    fn block_item_offers_tags() {
        assert_eq!(
            labels_at("---\ntags:\n  - prog|\n---\n").unwrap(),
            vec!["programming/cli", "programming/rust"]
        );
    }

    #[test]
    fn block_varying_indentation_still_resolves() {
        assert_eq!(
            labels_at("---\ntags:\n    - area|\n---\n").unwrap(),
            vec!["area/work"]
        );
    }

    #[test]
    fn block_second_item_resolves_through_the_first() {
        assert_eq!(
            labels_at("---\ntags:\n  - area/work\n  - prog|\n---\n").unwrap(),
            vec!["programming/cli", "programming/rust"]
        );
    }

    #[test]
    fn block_hyphenated_value_replaces_the_whole_token() {
        // Regression: a hyphen inside the value must not be mistaken for the
        // list dash. The replacement must start at the value, not mid-token.
        let custom = vec!["area/work-home".to_owned()];
        let text = "---\ntags:\n  - area/work-ho|\n---\n";
        let offset = text.find('|').unwrap();
        let text = text.replace('|', "");
        let list = complete(&text, offset, Encoding::Utf8, &custom).expect("context");
        assert_eq!(list.items[0].label, "area/work-home");
        let CompletionTextEdit::Edit(edit) = list.items[0].text_edit.as_ref().unwrap() else {
            panic!("edit");
        };
        // Line 2: "  - area/work-ho" — value starts at column 4, cursor at 16.
        assert_eq!(edit.range.start.line, 2);
        assert_eq!(edit.range.start.character, 4);
        assert_eq!(edit.range.end.character, 16);
    }

    #[test]
    fn block_hyphen_in_first_segment_resolves() {
        let custom = vec!["front-end/css".to_owned()];
        let text = "---\ntags:\n  - front-end/cs|\n---\n";
        let offset = text.find('|').unwrap();
        let text = text.replace('|', "");
        let list = complete(&text, offset, Encoding::Utf8, &custom).expect("context");
        assert_eq!(list.items[0].label, "front-end/css");
        let CompletionTextEdit::Edit(edit) = list.items[0].text_edit.as_ref().unwrap() else {
            panic!("edit");
        };
        // Value starts at column 4, after the "  - " list prefix.
        assert_eq!(edit.range.start.character, 4);
    }

    #[test]
    fn block_list_not_under_tags_declines() {
        assert!(labels_at("---\nauthors:\n  - prog|\n---\n").is_none());
    }

    #[test]
    fn block_blank_line_breaks_contiguity() {
        assert!(labels_at("---\ntags:\n\n  - prog|\n---\n").is_none());
    }

    #[test]
    fn cursor_on_the_tags_key_line_declines() {
        // `tags:` alone with the cursor at end is not a value position.
        assert!(labels_at("---\ntags:|\n  - rust\n---\n").is_none());
    }

    #[test]
    fn empty_tag_set_yields_an_empty_list() {
        let text = "---\ntags: [|]\n---\n";
        let offset = text.find('|').unwrap();
        let text = text.replace('|', "");
        let list = complete(&text, offset, Encoding::Utf8, &[]).expect("context");
        assert!(list.is_incomplete);
        assert!(list.items.is_empty());
    }

    #[test]
    fn replacement_range_covers_the_partial() {
        let text = "---\ntags: [prog|]\n---\n";
        let offset = text.find('|').unwrap();
        let text = text.replace('|', "");
        let list = complete(&text, offset, Encoding::Utf8, &tags()).expect("context");
        let CompletionTextEdit::Edit(edit) = list.items[0].text_edit.as_ref().unwrap() else {
            panic!("edit");
        };
        // Line 1: "tags: [prog" — partial starts at column 7, cursor at 11.
        assert_eq!(edit.range.start.line, 1);
        assert_eq!(edit.range.start.character, 7);
        assert_eq!(edit.range.end.character, 11);
    }
}
