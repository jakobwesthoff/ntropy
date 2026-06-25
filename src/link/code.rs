// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Masking of Markdown code regions (ADR 0028).
//!
//! Links inside fenced code blocks (```` ``` ````/`~~~`) and inline `` `code` ``
//! spans are documentation, not navigation, so link extraction and the
//! `reconcile` rewrite must ignore them. This module reports the byte ranges of
//! `body` that are code; everything else is open for link scanning.
//!
//! Indented (four-space) code blocks are deliberately **not** masked: a fence
//! indented four or more spaces is treated as ordinary text, matching the
//! documented limitation that links in indented code are still real links.

use std::ops::Range;

/// The byte ranges of `body` that are fenced or inline code.
///
/// Fenced ranges include the fence lines themselves; inline ranges include the
/// surrounding backticks. An unterminated fence masks everything to the end of
/// the document.
pub fn masked_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut fence: Option<Fence> = None;

    for line in lines(body) {
        let content = &body[line.start..line.content_end];
        match &fence {
            Some(open) => {
                // Inside a fence every line is code, including the closing one.
                ranges.push(line.start..line.next);
                if is_closing_fence(content, open) {
                    fence = None;
                }
            }
            None => match opening_fence(content) {
                Some(open) => {
                    ranges.push(line.start..line.next);
                    fence = Some(open);
                }
                None => mask_inline_spans(content, line.start, &mut ranges),
            },
        }
    }
    ranges
}

/// Whether the byte position falls within any masked range.
pub fn is_masked(masked: &[Range<usize>], position: usize) -> bool {
    masked.iter().any(|range| range.contains(&position))
}

/// An open fence: its marker character and the length of the opening run.
struct Fence {
    marker: u8,
    len: usize,
}

/// A line's byte boundaries: content (excluding `\n`) and the next line's start.
struct Line {
    start: usize,
    content_end: usize,
    next: usize,
}

/// Split `body` into lines on `\n`, retaining byte offsets. A trailing `\r`
/// stays in the line content (it is irrelevant to fence/backtick detection).
fn lines(body: &str) -> Vec<Line> {
    let mut spans = Vec::new();
    let bytes = body.as_bytes();
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            spans.push(Line {
                start,
                content_end: index,
                next: index + 1,
            });
            start = index + 1;
        }
    }
    spans.push(Line {
        start,
        content_end: bytes.len(),
        next: bytes.len(),
    });
    spans
}

/// Count of leading spaces, capped at four (a fence may be indented up to three).
fn leading_spaces(content: &str) -> usize {
    content.bytes().take_while(|&b| b == b' ').count()
}

/// If `content` opens a fenced block, return its fence.
fn opening_fence(content: &str) -> Option<Fence> {
    let indent = leading_spaces(content);
    if indent > 3 {
        return None;
    }
    let rest = &content.as_bytes()[indent..];
    let marker = *rest.first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = rest.iter().take_while(|&&b| b == marker).count();
    (len >= 3).then_some(Fence { marker, len })
}

/// Whether `content` closes the given open fence: an indented run of the same
/// marker, at least as long as the opener, followed only by whitespace.
fn is_closing_fence(content: &str, open: &Fence) -> bool {
    let indent = leading_spaces(content);
    if indent > 3 {
        return false;
    }
    let rest = &content.as_bytes()[indent..];
    let run = rest.iter().take_while(|&&b| b == open.marker).count();
    run >= open.len
        && rest[run..]
            .iter()
            .all(|&b| b == b' ' || b == b'\t' || b == b'\r')
}

/// Mask inline code spans within a single line's `content`.
///
/// A span runs from a backtick run to the next run of exactly equal length,
/// matching CommonMark. Spans confined to one line are handled; an unmatched run
/// is literal text and left unmasked.
fn mask_inline_spans(content: &str, base: usize, ranges: &mut Vec<Range<usize>>) {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let run_start = i;
        while i < bytes.len() && bytes[i] == b'`' {
            i += 1;
        }
        let run_len = i - run_start;
        if let Some(close_end) = closing_backtick_run(bytes, i, run_len) {
            ranges.push(base + run_start..base + close_end);
            i = close_end;
        }
    }
}

/// The end offset of the next backtick run of exactly `run_len`, searching from
/// `from`.
fn closing_backtick_run(bytes: &[u8], from: usize, run_len: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] == b'`' {
            i += 1;
        }
        if i - start == run_len {
            return Some(i);
        }
    }
    None
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
        // Four leading spaces is an indented code block, which we do not mask.
        let body = "    ```\nLINK";
        assert!(!masked(body, "LINK"));
    }

    #[test]
    fn three_space_indented_fence_is_a_fence() {
        let body = "   ```\nLINK\n   ```";
        assert!(masked(body, "LINK"));
    }
}
