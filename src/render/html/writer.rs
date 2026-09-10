// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The HTML output writer: one method per escaping context
//! (`docs/design/html-engine.md`, "Escaping").
//!
//! Escaping is a property of the write call, not of tracked emitter state.
//! Every character the emitter writes belongs to exactly one of three
//! contexts, text content, attribute values, and raw pass-through, and each
//! has a method here. Choosing the context lexically at the call site
//! removes the mode flag that would otherwise have to be switched and could
//! be forgotten.

/// The characters replaced in text content and attribute values. `&`, `<`,
/// and `>` are structural in text; `&` and the quotes are structural in a
/// quoted attribute. One set serves both contexts, so a value can move
/// between them without re-escaping.
const ESCAPED: [(char, &str); 5] = [
    ('&', "&amp;"),
    ('<', "&lt;"),
    ('>', "&gt;"),
    ('"', "&quot;"),
    ('\'', "&#39;"),
];

/// The HTML writer: one method per escaping context.
pub struct HtmlWriter {
    out: String,
}

impl HtmlWriter {
    pub fn new() -> Self {
        HtmlWriter { out: String::new() }
    }

    /// Context 1: text content between tags.
    pub fn text(&mut self, text: &str) {
        escape_into(&mut self.out, text);
    }

    /// Context 2: the value of a double-quoted attribute. The caller owns the
    /// surrounding quotes (via [`Self::syntax`]), because the quotes are HTML
    /// syntax, not user text.
    pub fn attribute(&mut self, value: &str) {
        escape_into(&mut self.out, value);
    }

    /// Context 3: raw pass-through. Raw HTML from the note body, and children
    /// already rendered by earlier writes.
    pub fn raw(&mut self, html: &str) {
        self.out.push_str(html);
    }

    /// Emitter-owned HTML syntax: tags, attribute names, the quotes around
    /// attribute values. This is the only unescaped channel besides `raw`,
    /// and its visibility is restricted to the `html` module so user-derived
    /// text cannot reach it from outside the emitter.
    pub(super) fn syntax(&mut self, s: &str) {
        self.out.push_str(s);
    }

    pub fn finish(self) -> String {
        self.out
    }
}

impl Default for HtmlWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// `text` escaped for text content or an attribute value, for callers that
/// assemble markup outside the emitter and need the same escaping.
pub fn escape(text: &str) -> String {
    let mut out = String::new();
    escape_into(&mut out, text);
    out
}

fn escape_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match ESCAPED.iter().find(|(from, _)| *from == c) {
            Some((_, to)) => out.push_str(to),
            None => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_escapes_the_five_structural_characters() {
        let mut writer = HtmlWriter::new();
        writer.text("a & b < c > d \" e ' f");
        assert_eq!(writer.finish(), "a &amp; b &lt; c &gt; d &quot; e &#39; f");
    }

    #[test]
    fn text_leaves_everything_else_untouched() {
        let mut writer = HtmlWriter::new();
        writer.text("Über Größe 日本語 #hash [bracket] $math$ \\slash\n");
        assert_eq!(
            writer.finish(),
            "Über Größe 日本語 #hash [bracket] $math$ \\slash\n"
        );
    }

    #[test]
    fn an_already_escaped_entity_is_escaped_again() {
        // The writer never guesses whether `&amp;` was meant literally; user
        // text that reads `&amp;` renders as `&amp;` on screen.
        let mut writer = HtmlWriter::new();
        writer.text("&amp;");
        assert_eq!(writer.finish(), "&amp;amp;");
    }

    #[test]
    fn an_attribute_value_cannot_close_its_quotes() {
        let mut writer = HtmlWriter::new();
        writer.syntax("<a href=\"");
        writer.attribute("x\" onclick=\"evil()");
        writer.syntax("\">");
        assert_eq!(writer.finish(), "<a href=\"x&quot; onclick=&quot;evil()\">");
    }

    #[test]
    fn escape_matches_the_writer_text_context() {
        let mut writer = HtmlWriter::new();
        writer.text("a<b>&\"'");
        assert_eq!(escape("a<b>&\"'"), writer.finish());
    }

    #[test]
    fn raw_and_syntax_pass_through_verbatim() {
        let mut writer = HtmlWriter::new();
        writer.syntax("<p>");
        writer.raw("<b>&</b>");
        writer.syntax("</p>");
        assert_eq!(writer.finish(), "<p><b>&</b></p>");
    }
}
