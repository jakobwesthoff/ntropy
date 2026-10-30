// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Masking of Markdown code regions (ADR 0028).
//!
//! Links inside code are documentation, not navigation, so link extraction and
//! the `reconcile` rewrite must ignore them. This module reports the byte ranges
//! of `body` that are code; everything else is open for link scanning.
//!
//! The ranges come from the Markdown parser the renderer uses, with the same
//! options, so a link is masked exactly when the rendered note shows it as
//! code: inline code spans (which may cross line breaks), fenced code blocks
//! and indented code blocks. Inside an HTML block nothing is code, because the
//! parser passes the block through as raw HTML.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag};

use crate::render::markdown::parser_options;

/// The byte ranges of `body` that are code.
///
/// Inline ranges include the surrounding backticks. Block ranges include the
/// fence lines of a fenced block. An unterminated fence runs to the end of the
/// document.
pub fn masked_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for (event, span) in Parser::new_ext(body, parser_options()).into_offset_iter() {
        // A block's `Start` event carries the span of the whole block, so its
        // `End` and the events inside it add nothing.
        match event {
            Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => ranges.push(span),
            _ => {}
        }
    }
    ranges
}

/// Whether the byte position falls within any masked range.
pub fn is_masked(masked: &[Range<usize>], position: usize) -> bool {
    masked.iter().any(|range| range.contains(&position))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masked(body: &str, needle: &str) -> bool {
        let position = body.find(needle).expect("needle present");
        is_masked(&masked_ranges(body), position)
    }

    #[test]
    fn backtick_fence_masks_its_contents() {
        let body = "before\n```\nLINK\n```\nafter";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "before"));
        assert!(!masked(body, "after"));
    }

    #[test]
    fn tilde_fence_masks_its_contents() {
        let body = "~~~\nLINK\n~~~";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn fence_with_info_string_is_still_a_fence() {
        let body = "```rust\nLINK\n```";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn unterminated_fence_masks_to_end_of_document() {
        let body = "```\nLINK\nstill code";
        assert!(masked(body, "LINK"));
        assert!(masked(body, "still code"));
    }

    #[test]
    fn text_after_a_closed_fence_is_not_masked() {
        let body = "```\ncode\n```\nLINK";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn inline_span_is_masked() {
        let body = "see `LINK` here";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "here"));
    }

    #[test]
    fn double_backtick_span_handles_inner_backtick() {
        let body = "``a`LINK`b`` rest";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "rest"));
    }

    #[test]
    fn unmatched_backtick_is_literal() {
        let body = "a ` LINK with no close";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn four_space_indented_fence_is_not_a_fence() {
        // Four leading spaces make an indented code block, not a fence, and
        // the block ends at the next unindented line.
        let body = "    ```\nLINK";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn three_space_indented_fence_is_a_fence() {
        let body = "   ```\nLINK\n   ```";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn inline_span_crossing_a_line_break_is_masked() {
        let body = "before `start\nLINK ends` after";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "before"));
        assert!(!masked(body, "after"));
    }

    #[test]
    fn inline_span_crossing_several_lines_is_masked() {
        let body = "`one\ntwo LINK\nthree` after";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "after"));
    }

    #[test]
    fn inline_span_crossing_a_crlf_break_is_masked() {
        let body = "before `start\r\nLINK ends` after";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "after"));
    }

    #[test]
    fn inline_span_with_multibyte_text_across_lines_is_masked() {
        let body = "ä `ö\nLINK ü` ß after";
        assert!(masked(body, "LINK"));
        assert!(!masked(body, "after"));
    }

    #[test]
    fn inline_span_does_not_cross_a_blank_line() {
        let body = "a `x\n\nLINK y` b";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn inline_span_does_not_cross_a_crlf_blank_line() {
        let body = "a `x\r\n\r\nLINK y` b";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn inline_span_does_not_cross_a_whitespace_only_line() {
        let body = "a `x\n  \t\nLINK y` b";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn inline_span_does_not_cross_a_fence() {
        let body = "a `x\n```\ncode\n```\nLINK `";
        assert!(masked(body, "code"));
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn lone_backtick_in_a_list_item_does_not_swallow_the_next_item() {
        let body = "- press `\n- see LINK and `x`";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn lone_backtick_in_a_heading_does_not_swallow_the_next_line() {
        let body = "## Heading `\nsee LINK and `x`";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn lone_backtick_in_a_table_row_does_not_swallow_the_next_row() {
        let body = "| a | b ` |\n| - | - |\n| LINK | `x` |";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn escaped_backticks_do_not_open_a_span() {
        let body = "a \\`LINK\\` b";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn backtick_fence_info_string_may_not_contain_a_backtick() {
        // A line starting with a backtick run and a later backtick is an inline
        // span (or literal text), never a fence.
        let body = "``` x ` y\nLINK";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn fence_inside_a_list_item_is_masked() {
        let body = "- item\n\n  ```\n  LINK\n  ```\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn fence_inside_a_block_quote_is_masked() {
        let body = "> ```\n> LINK\n> ```\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn indented_code_block_is_masked() {
        let body = "text\n\n    [x](LINK)\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn indented_list_continuation_is_not_code() {
        // Four spaces under a two-column list marker is still item text.
        let body = "- item\n\n    LINK continues\n";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn code_text_inside_an_html_block_is_not_masked() {
        // The parser passes an HTML block through as raw HTML, backticks
        // included.
        let body = "<details>\n`LINK`\n</details>\n";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn code_span_after_a_blank_line_in_an_html_block_is_masked() {
        // A blank line ends this kind of HTML block, so Markdown applies again.
        let body = "<details>\n\n`LINK`\n\n</details>\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn indented_line_continuing_a_paragraph_is_not_code() {
        let body = "text\n    [x](LINK)\n";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn indented_line_after_a_heading_is_code() {
        let body = "## Heading\n    [x](LINK)\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn indented_code_inside_a_list_item_is_masked() {
        let body = "- item\n\n      [x](LINK)\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn indented_code_inside_a_block_quote_is_masked() {
        let body = ">     [x](LINK)\n";
        assert!(masked(body, "LINK"));
    }

    #[test]
    fn tab_indented_code_block_is_masked() {
        let body = "text\n\n\t[x](LINK)\n";
        assert!(masked(body, "LINK"));
    }
}
