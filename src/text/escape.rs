// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Escaping text that ntropy did not write before it is shown.

use std::borrow::Cow;

/// `raw` with its control characters escaped, for showing text that ntropy
/// did not write, such as a name that has not been validated.
///
/// Each escaped character is written the way `char::escape_debug` writes it
/// (`\n`, `\t`, `\u{1b}`), so an escape sequence in hostile text never
/// reaches the terminal. Every other character stays as written, a backslash
/// or quote included. Text with nothing to escape is borrowed unchanged.
pub fn escape_controls(raw: &str) -> Cow<'_, str> {
    if !raw.chars().any(needs_escape) {
        return Cow::Borrowed(raw);
    }
    let mut escaped = String::with_capacity(raw.len() + 8);
    for ch in raw.chars() {
        if needs_escape(ch) {
            escaped.extend(ch.escape_debug());
        } else {
            escaped.push(ch);
        }
    }
    Cow::Owned(escaped)
}

/// Whether `ch` is shown escaped.
fn needs_escape(ch: char) -> bool {
    ch.is_control()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_controls_escapes_only_control_characters() {
        assert_eq!(escape_controls("by\ntag\x1b[2J"), "by\\ntag\\u{1b}[2J");
        assert_eq!(escape_controls("it's a\\b"), "it's a\\b");
    }

    #[test]
    fn clean_text_is_borrowed() {
        assert!(matches!(
            escape_controls("Quarterly review"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn characters_that_render_stay_as_they_are() {
        for text in [
            "日本語",
            // A combining acute accent.
            "e\u{301}",
            // A family emoji joined by zero-width joiners.
            "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
            // The flag of England: a black flag followed by tag characters.
            "\u{1f3f4}\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}",
            "a b",
            "a\\b",
            "it's",
            "\"q\"",
            // The directional marks mixed-script text needs.
            "\u{200e}\u{200f}\u{61c}",
            // The zero-width non-joiner and joiner.
            "\u{200c}\u{200d}",
        ] {
            assert_eq!(escape_controls(text), text, "{}", text.escape_debug());
        }
    }

    #[test]
    fn c1_and_del_are_escaped() {
        assert_eq!(escape_controls("\u{7f}"), "\\u{7f}");
        assert_eq!(escape_controls("\u{85}"), "\\u{85}");
        assert_eq!(escape_controls("\u{9b}"), "\\u{9b}");
    }

    #[test]
    fn an_osc_52_payload_loses_its_escape_bytes() {
        let escaped = escape_controls("\u{1b}]52;c;ZXZpbAo=\u{7}");
        assert!(!escaped.contains(['\u{1b}', '\u{7}']), "{escaped}");
        assert_eq!(escaped, "\\u{1b}]52;c;ZXZpbAo=\\u{7}");
    }

    #[test]
    fn escaping_is_idempotent() {
        // The escapes other display code nests: a note reference inside an
        // error message, a skipped view inside a warning.
        let once = escape_controls("a\x1b[2J\tb\u{85}\u{202e}c\nd");
        assert_eq!(escape_controls(&once), once);
    }
}
