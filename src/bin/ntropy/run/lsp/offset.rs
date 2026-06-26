// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Byte-offset ↔ LSP `Position` conversion under the negotiated position
//! encoding (ADR 0029, `docs/design/language-server.md`).
//!
//! LSP positions are `(line, character)` pairs where the unit of `character`
//! depends on the encoding negotiated at `initialize`: UTF-8 counts bytes,
//! UTF-16 counts UTF-16 code units. ntropy works in byte offsets internally, so
//! every range exchanged with the client crosses this boundary exactly once,
//! here.

use lsp_types::Position;

/// The position encoding negotiated during `initialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// `character` counts UTF-8 code units (bytes).
    Utf8,
    /// `character` counts UTF-16 code units. The protocol default.
    Utf16,
}

/// Byte offsets at which each line begins.
///
/// Lines are split on `\n` only; a `\r` of a CRLF pair stays part of the line,
/// because LSP positions are measured over the raw buffer the client holds, not
/// a normalized one.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

/// The byte offset of an LSP `Position` within `text`.
///
/// A line beyond the end clamps to the end of the text, and a `character`
/// beyond the line clamps to the line's end (excluding its `\n`), matching the
/// protocol's "defaults back to the line length" rule.
pub fn position_to_offset(text: &str, position: Position, encoding: Encoding) -> usize {
    let starts = line_starts(text);
    let line = position.line as usize;
    let Some(&line_start) = starts.get(line) else {
        return text.len();
    };
    // The line's content runs up to the next line's start minus its `\n`, or to
    // the end of the text for the final line.
    let line_end = starts.get(line + 1).map_or(text.len(), |&next| next - 1);
    let line_text = &text[line_start..line_end];
    let character = position.character as usize;
    let within = match encoding {
        Encoding::Utf8 => clamp_utf8(line_text, character),
        Encoding::Utf16 => offset_for_utf16_units(line_text, character),
    };
    line_start + within
}

/// The LSP `Position` of a byte offset within `text`.
///
/// The offset is clamped to the text length and rounded down to a `char`
/// boundary so the result always refers to a real position.
pub fn offset_to_position(text: &str, offset: usize, encoding: Encoding) -> Position {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let starts = line_starts(text);
    // The line is the last one whose start is at or before the offset.
    let line = match starts.binary_search(&offset) {
        Ok(index) => index,
        Err(index) => index - 1,
    };
    let line_start = starts[line];
    let slice = &text[line_start..offset];
    let character = match encoding {
        Encoding::Utf8 => slice.len(),
        Encoding::Utf16 => slice.chars().map(char::len_utf16).sum(),
    };
    Position::new(line as u32, character as u32)
}

/// Clamp a UTF-8 `character` (a byte count) to the line, rounding down to a
/// `char` boundary.
fn clamp_utf8(line: &str, character: usize) -> usize {
    if character >= line.len() {
        return line.len();
    }
    let mut byte = character;
    while byte > 0 && !line.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// The byte offset within `line` reached after `units` UTF-16 code units,
/// clamped to the line's end.
fn offset_for_utf16_units(line: &str, units: usize) -> usize {
    let mut seen = 0;
    for (byte_index, ch) in line.char_indices() {
        // Stop at the char boundary that reaches `units`. The `>` also catches a
        // `units` that falls *inside* this char's surrogate pair (an out-of-spec
        // position): clamp it to the char's start rather than overshooting.
        if seen + ch.len_utf16() > units {
            return byte_index;
        }
        seen += ch.len_utf16();
    }
    line.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, character: u32) -> Position {
        Position::new(line, character)
    }

    #[test]
    fn ascii_round_trips_in_both_encodings() {
        let text = "hello world";
        for enc in [Encoding::Utf8, Encoding::Utf16] {
            assert_eq!(position_to_offset(text, pos(0, 5), enc), 5);
            assert_eq!(offset_to_position(text, 5, enc), pos(0, 5));
        }
    }

    #[test]
    fn umlaut_differs_by_encoding() {
        // "ä" is 2 UTF-8 bytes but 1 UTF-16 unit. Cursor just after the "ä".
        let text = "äx";
        assert_eq!(position_to_offset(text, pos(0, 2), Encoding::Utf8), 2);
        assert_eq!(position_to_offset(text, pos(0, 1), Encoding::Utf16), 2);
        assert_eq!(offset_to_position(text, 2, Encoding::Utf8), pos(0, 2));
        assert_eq!(offset_to_position(text, 2, Encoding::Utf16), pos(0, 1));
    }

    #[test]
    fn four_byte_char_counts_two_utf16_units() {
        // "😀" is 4 UTF-8 bytes and 2 UTF-16 units (a surrogate pair).
        let text = "😀!";
        assert_eq!(position_to_offset(text, pos(0, 4), Encoding::Utf8), 4);
        assert_eq!(position_to_offset(text, pos(0, 2), Encoding::Utf16), 4);
        assert_eq!(offset_to_position(text, 4, Encoding::Utf8), pos(0, 4));
        assert_eq!(offset_to_position(text, 4, Encoding::Utf16), pos(0, 2));
    }

    #[test]
    fn utf16_position_inside_a_surrogate_pair_clamps_to_the_char_start() {
        // `character = 1` points into the high surrogate of "😀", an out-of-spec
        // position. It must clamp to the char's start (byte 0), not overshoot.
        assert_eq!(position_to_offset("😀", pos(0, 1), Encoding::Utf16), 0);
        assert_eq!(position_to_offset("😀!", pos(0, 1), Encoding::Utf16), 0);
        // The valid boundary just after the emoji still resolves to the "!".
        assert_eq!(position_to_offset("😀!", pos(0, 2), Encoding::Utf16), 4);
    }

    #[test]
    fn multi_line_resolves_against_the_right_line() {
        let text = "abc\ndefg\nhi";
        assert_eq!(position_to_offset(text, pos(1, 2), Encoding::Utf8), 6);
        assert_eq!(offset_to_position(text, 6, Encoding::Utf8), pos(1, 2));
        // Start of the third line.
        assert_eq!(position_to_offset(text, pos(2, 0), Encoding::Utf8), 9);
        assert_eq!(offset_to_position(text, 9, Encoding::Utf8), pos(2, 0));
    }

    #[test]
    fn crlf_keeps_carriage_return_in_the_line() {
        let text = "ab\r\ncd";
        // The `\r` is the third character of line 0; the offset of the `\n`.
        assert_eq!(position_to_offset(text, pos(0, 3), Encoding::Utf8), 3);
        // Line 1 begins after the `\n` at byte 4.
        assert_eq!(position_to_offset(text, pos(1, 0), Encoding::Utf8), 4);
        assert_eq!(offset_to_position(text, 4, Encoding::Utf8), pos(1, 0));
    }

    #[test]
    fn empty_document_is_origin() {
        assert_eq!(position_to_offset("", pos(0, 0), Encoding::Utf8), 0);
        assert_eq!(offset_to_position("", 0, Encoding::Utf8), pos(0, 0));
    }

    #[test]
    fn character_past_end_of_line_clamps() {
        let text = "abc\ndef";
        assert_eq!(position_to_offset(text, pos(0, 99), Encoding::Utf8), 3);
        assert_eq!(position_to_offset(text, pos(0, 99), Encoding::Utf16), 3);
    }

    #[test]
    fn line_past_end_clamps_to_text_length() {
        let text = "abc";
        assert_eq!(position_to_offset(text, pos(9, 0), Encoding::Utf8), 3);
    }

    #[test]
    fn offset_past_end_clamps() {
        let text = "abc";
        assert_eq!(offset_to_position(text, 99, Encoding::Utf8), pos(0, 3));
    }

    #[test]
    fn round_trips_on_every_char_boundary() {
        let text = "aä😀b\ncäd\r\n😀";
        for enc in [Encoding::Utf8, Encoding::Utf16] {
            for offset in 0..=text.len() {
                if !text.is_char_boundary(offset) {
                    continue;
                }
                let position = offset_to_position(text, offset, enc);
                assert_eq!(
                    position_to_offset(text, position, enc),
                    offset,
                    "round-trip failed at offset {offset} under {enc:?}"
                );
            }
        }
    }
}
