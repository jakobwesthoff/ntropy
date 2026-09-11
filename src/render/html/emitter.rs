// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The HTML output of the shared Markdown walk: turns a note body into an
//! HTML body fragment (`docs/design/html-engine.md`, "Constructs"). The walk
//! itself, and everything that is a property of the Markdown structure
//! rather than of HTML, lives in [`crate::render::markdown`].
//!
//! # Element mapping
//!
//! Block constructs: `<p>`, `<h1>` to `<h6>`, `<pre><code>` with a
//! `language-<tag>` class taken from the fence info string, `<ul>` and
//! `<ol start="n">`, `<blockquote>`, GFM callouts as
//! `<div class="callout callout-<kind>">` with a `<p class="callout-title">`
//! opening on a `<use>` of the kind's icon from the page's sprite,
//! `<table>` with `<thead>` and `<tbody>` and per-column `text-align` styles,
//! `<hr>`.
//!
//! Inline constructs: `<em>`, `<strong>`, `<del>`, `<code>`, `<br>`, a soft
//! break as a newline in the text, `<a href>` for ordinary links and
//! autolinks, `<a class="note-link" href>` carrying the target's title for a
//! resolved note link, `<img src alt>` for images local and remote alike, a
//! task-list checkbox as a disabled `<input type="checkbox">`.
//!
//! Footnotes become a numbered `<sup class="footnote-ref">` at each reference
//! and a `<section class="footnotes">` appended after the body, in first-
//! reference order. Raw HTML, blocks and inline fragments alike, passes
//! through verbatim.
//!
//! # What the emitter records
//!
//! Besides the fragment, [`emit`] returns the local files the body
//! references (image sources and link targets without a scheme), so the
//! site export copies exactly those, and the fence languages it saw, so a
//! page loads only the grammars it needs (ADR 0053). Where those references
//! point at in the artifact is the caller's decision, expressed through
//! [`Targets`].

use super::writer::HtmlWriter;
use crate::id::Id;
use crate::render::ResolvedLink;
use crate::render::markdown::{
    self, Alignment, BlockQuoteKind, Heading, InlineStyle, List, Output, Table, Warning,
};

// =========================================================
// Public entry
// =========================================================

/// Where the artifact's links point. The emitter decides *what* a note link
/// or an asset reference is; the caller decides the `href` or `src` it
/// becomes, because that depends on where the artifact lands relative to
/// other artifacts and to the vault.
pub trait Targets {
    /// The `href` of a resolved note link.
    fn note_href(&self, id: Id, slug: &str) -> String;

    /// The `src` or `href` for a local path as written in the note body.
    fn asset_href(&self, dest: &str) -> String;
}

/// The convention of a single-note `render`: the target note's artifact sits
/// beside this one under its default name, `<slug>.html` (the HTML
/// counterpart of ADR 0044), and asset paths are left as written.
pub struct SiblingArtifacts;

impl Targets for SiblingArtifacts {
    fn note_href(&self, _id: Id, slug: &str) -> String {
        format!("{slug}.html")
    }

    fn asset_href(&self, dest: &str) -> String {
        dest.to_string()
    }
}

/// The result of converting one note body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emitted {
    pub html: String,
    pub warnings: Vec<Warning>,
    /// Local paths the body references, as written, each once, in order of
    /// first appearance.
    pub assets: Vec<String>,
    /// Fence languages the body's code blocks carry, each once, in order of
    /// first appearance.
    pub languages: Vec<String>,
    /// The body's headings in document order, with the ids the fragment
    /// carries, for an outline.
    pub headings: Vec<Heading>,
}

/// Convert a note body to an HTML body fragment, resolving note links against
/// `links` (ADR 0028) and pointing links where `targets` says.
///
/// A page shows the note's title in its header, so a body that opens with a
/// level-one heading reading exactly `title` would show it twice; the walk
/// drops that heading from the fragment and the outline never sees it.
pub fn emit(
    body: &str,
    links: &[ResolvedLink],
    targets: &dyn Targets,
    title: Option<&str>,
) -> Emitted {
    let mut output = HtmlOutput {
        targets,
        title: title.map(|title| title.trim().to_string()),
        assets: Vec::new(),
        languages: Vec::new(),
        headings: Vec::new(),
        footnotes: Vec::new(),
    };
    let (html, warnings) = markdown::walk(body, links, &mut output);
    Emitted {
        html,
        warnings,
        assets: output.assets,
        languages: output.languages,
        headings: output.headings,
    }
}

// =========================================================
// The HTML output
// =========================================================

struct HtmlOutput<'a> {
    targets: &'a dyn Targets,
    /// The title a leading level-one heading must equal to be dropped.
    title: Option<String>,
    assets: Vec<String>,
    languages: Vec<String>,
    headings: Vec<Heading>,
    /// The rendered content of every footnote the walk handed over, in
    /// first-reference order; the position is the footnote's number.
    footnotes: Vec<String>,
}

impl HtmlOutput<'_> {
    /// Record a local reference once, in order of first appearance.
    fn record_asset(&mut self, dest: &str) {
        if !self.assets.iter().any(|seen| seen == dest) {
            self.assets.push(dest.to_string());
        }
    }

    fn record_language(&mut self, language: &str) {
        if !self.languages.iter().any(|seen| seen == language) {
            self.languages.push(language.to_string());
        }
    }

    /// The `href` for a link destination: a local path goes through the
    /// targets and is recorded as an asset, anything else is left as written.
    fn link_href(&mut self, dest: &str) -> String {
        if is_local(dest) {
            self.record_asset(dest);
            self.targets.asset_href(dest)
        } else {
            dest.to_string()
        }
    }
}

impl Output for HtmlOutput<'_> {
    fn text(&mut self, text: &str) -> String {
        let mut writer = HtmlWriter::new();
        writer.text(text);
        writer.finish()
    }

    fn inline_code(&mut self, code: &str) -> String {
        let mut writer = HtmlWriter::new();
        writer.syntax("<code>");
        writer.text(code);
        writer.syntax("</code>");
        writer.finish()
    }

    fn soft_break(&mut self) -> String {
        "\n".to_string()
    }

    fn hard_break(&mut self) -> String {
        "<br>\n".to_string()
    }

    fn task_marker(&mut self, checked: bool) -> String {
        if checked {
            "<input type=\"checkbox\" disabled checked> ".to_string()
        } else {
            "<input type=\"checkbox\" disabled> ".to_string()
        }
    }

    fn styled(&mut self, style: InlineStyle, inner: &str) -> String {
        let tag = match style {
            InlineStyle::Emph => "em",
            InlineStyle::Strong => "strong",
            InlineStyle::Strike => "del",
        };
        format!("<{tag}>{inner}</{tag}>")
    }

    fn link(&mut self, dest: &str, inner: &str) -> String {
        let href = self.link_href(dest);
        let mut writer = HtmlWriter::new();
        writer.syntax("<a href=\"");
        writer.attribute(&href);
        writer.syntax("\">");
        writer.raw(inner);
        writer.syntax("</a>");
        writer.finish()
    }

    fn note_link(&mut self, id: Id, title: &str, slug: &str) -> String {
        let href = self.targets.note_href(id, slug);
        let mut writer = HtmlWriter::new();
        writer.syntax("<a class=\"note-link\" href=\"");
        writer.attribute(&href);
        writer.syntax("\">");
        writer.text(title);
        writer.syntax("</a>");
        writer.finish()
    }

    /// Local and remote images alike become `<img>`; a browser fetches a
    /// remote one itself, so nothing degrades.
    fn image(&mut self, dest: &str, alt: &str, _warnings: &mut Vec<Warning>) -> String {
        let src = self.link_href(dest);
        let mut writer = HtmlWriter::new();
        writer.syntax("<img src=\"");
        writer.attribute(&src);
        writer.syntax("\" alt=\"");
        writer.attribute(alt);
        writer.syntax("\">");
        writer.finish()
    }

    /// The reference marker; the content is kept for the section `finish`
    /// appends. Every reference to one footnote shares the marker, the
    /// walk substituting it at each site, so the back-link from the section
    /// returns to the first reference.
    fn footnote(&mut self, content: &str) -> String {
        self.footnotes.push(content.to_string());
        let number = self.footnotes.len();
        format!(
            "<sup class=\"footnote-ref\"><a href=\"#fn-{number}\" id=\"fnref-{number}\">{number}</a></sup>"
        )
    }

    fn inline_html(&mut self, html: &str, _warnings: &mut Vec<Warning>) -> Option<String> {
        Some(html.to_string())
    }

    fn paragraph(&mut self, body: &str) -> String {
        format!("<p>{body}</p>\n")
    }

    fn dropped_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn heading(&mut self, heading: &Heading, body: &str) -> String {
        self.headings.push(heading.clone());
        let level = heading.level as usize;
        let mut writer = HtmlWriter::new();
        writer.syntax(&format!("<h{level} id=\""));
        writer.attribute(&heading.id);
        writer.syntax("\">");
        writer.raw(body);
        writer.syntax(&format!("</h{level}>\n"));
        writer.finish()
    }

    fn quote(&mut self, kind: Option<BlockQuoteKind>, body: &str) -> String {
        match kind {
            None => format!("<blockquote>\n{body}</blockquote>\n"),
            Some(kind) => {
                let (class, title, icon) = callout(kind);
                format!(
                    "<div class=\"callout callout-{class}\">\n<p class=\"callout-title\"><svg class=\"icon\" aria-hidden=\"true\"><use href=\"#icon-{icon}\"/></svg>{title}</p>\n{body}</div>\n"
                )
            }
        }
    }

    /// The info string's first token is the language, escaped as an
    /// attribute; any token shape is acceptable because nothing parses the
    /// class beyond the highlighter's lookup.
    fn code_block(&mut self, info: Option<&str>, content: &str) -> String {
        let language = info.and_then(|info| info.split_whitespace().next());
        let mut writer = HtmlWriter::new();
        writer.syntax("<pre><code");
        if let Some(language) = language {
            self.record_language(language);
            writer.syntax(" class=\"language-");
            writer.attribute(language);
            writer.syntax("\"");
        }
        writer.syntax(">");
        writer.text(content);
        writer.syntax("</code></pre>\n");
        writer.finish()
    }

    /// Tight and loose lists need no distinction here: the walk wraps a loose
    /// item's content in paragraphs already, so the item body carries it.
    fn list(&mut self, list: &List) -> String {
        let mut out = String::new();
        match list.start {
            None => out.push_str("<ul>\n"),
            Some(1) => out.push_str("<ol>\n"),
            Some(start) => out.push_str(&format!("<ol start=\"{start}\">\n")),
        }
        for item in &list.items {
            // A body ending in a newline holds blocks and reads better with
            // the closing tag on its own line; inline content stays on one.
            if item.ends_with('\n') {
                out.push_str(&format!("<li>\n{item}</li>\n"));
            } else {
                out.push_str(&format!("<li>{item}</li>\n"));
            }
        }
        out.push_str(if list.start.is_none() {
            "</ul>\n"
        } else {
            "</ol>\n"
        });
        out
    }

    fn table(&mut self, table: &Table) -> String {
        let mut out = String::from("<table>\n<thead>\n<tr>\n");
        for (column, cell) in table.header.iter().enumerate() {
            out.push_str(&cell_tag("th", table.alignments.get(column), cell));
        }
        out.push_str("</tr>\n</thead>\n<tbody>\n");
        for row in &table.rows {
            out.push_str("<tr>\n");
            for (column, cell) in row.iter().enumerate() {
                out.push_str(&cell_tag("td", table.alignments.get(column), cell));
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody>\n</table>\n");
        out
    }

    fn rule(&mut self) -> String {
        "<hr>\n".to_string()
    }

    fn html_block(&mut self, html: &str, _warnings: &mut Vec<Warning>) -> Option<String> {
        Some(html.to_string())
    }

    /// Append the footnote section. The contents were rendered by the walk
    /// and may hold references of their own; the walk patches those after
    /// this returns.
    fn finish(&mut self, body: String) -> String {
        if self.footnotes.is_empty() {
            return body;
        }
        let mut out = body;
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("<section class=\"footnotes\">\n<ol>\n");
        for (index, content) in self.footnotes.iter().enumerate() {
            let number = index + 1;
            out.push_str(&format!(
                "<li id=\"fn-{number}\">\n{content}<a class=\"footnote-backref\" href=\"#fnref-{number}\">↩</a>\n</li>\n"
            ));
        }
        out.push_str("</ol>\n</section>\n");
        out
    }
}

/// The class suffix, the visible title, and the sprite icon of a GFM
/// callout kind (the icon names are files of the theme's `icons/`).
fn callout(kind: BlockQuoteKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        BlockQuoteKind::Note => ("note", "Note", "info"),
        BlockQuoteKind::Tip => ("tip", "Tip", "lightbulb"),
        BlockQuoteKind::Important => ("important", "Important", "message-square-warning"),
        BlockQuoteKind::Warning => ("warning", "Warning", "triangle-alert"),
        BlockQuoteKind::Caution => ("caution", "Caution", "octagon-alert"),
    }
}

/// One table cell with its column's alignment as an inline style; a column
/// without an explicit alignment gets no style.
fn cell_tag(tag: &str, alignment: Option<&Alignment>, content: &str) -> String {
    let style = match alignment {
        Some(Alignment::Left) => " style=\"text-align: left\"",
        Some(Alignment::Center) => " style=\"text-align: center\"",
        Some(Alignment::Right) => " style=\"text-align: right\"",
        _ => "",
    };
    format!("<{tag}{style}>{content}</{tag}>\n")
}

/// Whether a destination names a file rather than a URL or an in-page
/// anchor: no scheme, not protocol-relative, not a fragment, not empty.
fn is_local(dest: &str) -> bool {
    if dest.is_empty() || dest.starts_with('#') || dest.starts_with("//") {
        return false;
    }
    !has_scheme(dest)
}

/// A URI scheme per RFC 3986: a letter, then letters, digits, `+`, `-`, `.`,
/// then a colon.
fn has_scheme(dest: &str) -> bool {
    let mut chars = dest.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Range;

    use crate::render::LinkTarget;

    fn html(input: &str) -> String {
        emit(input, &[], &SiblingArtifacts, None).html
    }

    fn emitted(input: &str) -> Emitted {
        emit(input, &[], &SiblingArtifacts, None)
    }

    fn note_link(range: Range<usize>, target: Option<(&str, &str)>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: Id::from_timestamp_ms(0),
            target: target.map(|(title, slug)| LinkTarget {
                title: title.to_string(),
                slug: slug.to_string(),
            }),
        }
    }

    // =====================================================================
    // Paragraphs, headings, breaks
    // =====================================================================

    #[test]
    fn empty_body_emits_nothing() {
        assert_eq!(html(""), "");
    }

    #[test]
    fn a_paragraph_escapes_its_text() {
        assert_eq!(
            html("Tom & Jerry <3 \"quotes\""),
            "<p>Tom &amp; Jerry &lt;3 &quot;quotes&quot;</p>\n"
        );
    }

    #[test]
    fn adjacent_paragraphs_are_separated_by_a_blank_line() {
        assert_eq!(html("a\n\nb"), "<p>a</p>\n\n<p>b</p>\n");
    }

    #[test]
    fn headings_of_every_level_carry_an_id() {
        assert_eq!(
            html("# 1\n## 2\n### 3\n#### 4\n##### 5\n###### 6"),
            "<h1 id=\"1\">1</h1>\n\n<h2 id=\"2\">2</h2>\n\n<h3 id=\"3\">3</h3>\n\n<h4 id=\"4\">4</h4>\n\n<h5 id=\"5\">5</h5>\n\n<h6 id=\"6\">6</h6>\n"
        );
    }

    #[test]
    fn a_leading_h1_equal_to_the_title_is_dropped_with_its_outline_entry() {
        let out = emit(
            "# Rust Tips\n\nBody.\n\n# Rust Tips\n",
            &[],
            &SiblingArtifacts,
            Some("Rust Tips"),
        );
        assert_eq!(
            out.html,
            "<p>Body.</p>\n\n<h1 id=\"rust-tips\">Rust Tips</h1>\n"
        );
        assert_eq!(out.headings.len(), 1);
        assert_eq!(out.headings[0].id, "rust-tips");
    }

    #[test]
    fn a_title_heading_stays_when_it_differs_or_is_not_first() {
        let kept = |body: &str| emit(body, &[], &SiblingArtifacts, Some("Rust Tips")).html;
        assert_eq!(
            kept("# Rust tips\n"),
            "<h1 id=\"rust-tips\">Rust tips</h1>\n"
        );
        assert_eq!(
            kept("## Rust Tips\n"),
            "<h2 id=\"rust-tips\">Rust Tips</h2>\n"
        );
        assert_eq!(
            kept("Intro.\n\n# Rust Tips\n"),
            "<p>Intro.</p>\n\n<h1 id=\"rust-tips\">Rust Tips</h1>\n"
        );
        assert_eq!(
            emit("# Rust Tips\n", &[], &SiblingArtifacts, None).html,
            "<h1 id=\"rust-tips\">Rust Tips</h1>\n"
        );
    }

    #[test]
    fn headings_are_recorded_for_the_outline_with_flattened_text() {
        let out = emitted("# Intro *now*\n\n## Table\n\n## Table");
        assert_eq!(
            out.headings,
            vec![
                Heading {
                    level: markdown::HeadingLevel::H1,
                    id: "intro-now".to_string(),
                    text: "Intro now".to_string(),
                },
                Heading {
                    level: markdown::HeadingLevel::H2,
                    id: "table".to_string(),
                    text: "Table".to_string(),
                },
                Heading {
                    level: markdown::HeadingLevel::H2,
                    id: "table-1".to_string(),
                    text: "Table".to_string(),
                },
            ]
        );
        assert!(out.html.contains("<h2 id=\"table-1\">Table</h2>"));
    }

    #[test]
    fn a_heading_id_is_escaped_as_an_attribute() {
        // The slug rule leaves no characters that need escaping, so this
        // pins that the attribute context is used regardless.
        assert_eq!(html("# a\"b"), "<h1 id=\"ab\">a&quot;b</h1>\n");
    }

    #[test]
    fn soft_breaks_are_newlines_and_hard_breaks_are_br() {
        assert_eq!(html("one\ntwo  \nthree"), "<p>one\ntwo<br>\nthree</p>\n");
    }

    #[test]
    fn thematic_break() {
        assert_eq!(html("a\n\n---\n\nb"), "<p>a</p>\n\n<hr>\n\n<p>b</p>\n");
    }

    // =====================================================================
    // Inline styling and code
    // =====================================================================

    #[test]
    fn emphasis_strong_and_strikethrough_nest() {
        assert_eq!(
            html("*a **b** ~~c~~*"),
            "<p><em>a <strong>b</strong> <del>c</del></em></p>\n"
        );
    }

    #[test]
    fn inline_code_is_escaped_text_inside_code() {
        assert_eq!(
            html("`let x = a < b && c;`"),
            "<p><code>let x = a &lt; b &amp;&amp; c;</code></p>\n"
        );
    }

    // =====================================================================
    // Code blocks
    // =====================================================================

    #[test]
    fn a_fenced_block_carries_its_language_class_and_records_it() {
        let out = emitted("```rust ignore\nfn x() -> &str {}\n```");
        assert_eq!(
            out.html,
            "<pre><code class=\"language-rust\">fn x() -&gt; &amp;str {}\n</code></pre>\n"
        );
        assert_eq!(out.languages, vec!["rust"]);
    }

    #[test]
    fn a_fence_without_a_language_gets_no_class() {
        let out = emitted("```\nplain\n```");
        assert_eq!(out.html, "<pre><code>plain\n</code></pre>\n");
        assert!(out.languages.is_empty());
    }

    #[test]
    fn an_indented_block_has_no_language() {
        // An indented block's final line carries no newline of its own.
        assert_eq!(html("    x = 1"), "<pre><code>x = 1</code></pre>\n");
    }

    #[test]
    fn a_language_is_recorded_once_and_escaped_as_an_attribute() {
        let out = emitted("```c\na\n```\n\n```c\nb\n```\n\n```a\"b\nc\n```");
        assert_eq!(out.languages, vec!["c", "a\"b"]);
        assert!(out.html.contains("class=\"language-a&quot;b\""));
    }

    #[test]
    fn code_content_is_never_parsed_as_html() {
        assert_eq!(
            html("```\n<script>alert(1)</script>\n```"),
            "<pre><code>&lt;script&gt;alert(1)&lt;/script&gt;\n</code></pre>\n"
        );
    }

    // =====================================================================
    // Lists
    // =====================================================================

    #[test]
    fn tight_bullet_list() {
        assert_eq!(html("- a\n- b"), "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n");
    }

    #[test]
    fn loose_list_wraps_items_in_paragraphs() {
        assert_eq!(
            html("- a\n\n- b"),
            "<ul>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n</ul>\n"
        );
    }

    #[test]
    fn ordered_list_starting_at_one_has_no_start_attribute() {
        assert_eq!(html("1. a\n2. b"), "<ol>\n<li>a</li>\n<li>b</li>\n</ol>\n");
    }

    #[test]
    fn ordered_list_keeps_an_explicit_start() {
        assert_eq!(
            html("6. a\n7. b"),
            "<ol start=\"6\">\n<li>a</li>\n<li>b</li>\n</ol>\n"
        );
    }

    #[test]
    fn nested_lists_sit_inside_their_item() {
        insta::assert_snapshot!(html("- a\n  - b\n    - c\n- d"));
    }

    #[test]
    fn task_list_markers_become_disabled_checkboxes() {
        assert_eq!(
            html("- [x] done\n- [ ] open"),
            "<ul>\n<li><input type=\"checkbox\" disabled checked> done</li>\n<li><input type=\"checkbox\" disabled> open</li>\n</ul>\n"
        );
    }

    #[test]
    fn a_continuation_paragraph_stays_in_its_item() {
        insta::assert_snapshot!(html("- first\n\n  continued\n\n- second"));
    }

    // =====================================================================
    // Quotes and callouts
    // =====================================================================

    #[test]
    fn block_quote_with_two_paragraphs() {
        assert_eq!(
            html("> a\n>\n> b"),
            "<blockquote>\n<p>a</p>\n\n<p>b</p>\n</blockquote>\n"
        );
    }

    #[test]
    fn all_five_callout_kinds() {
        insta::assert_snapshot!(html(
            "> [!NOTE]\n> n\n\n> [!TIP]\n> t\n\n> [!IMPORTANT]\n> i\n\n> [!WARNING]\n> w\n\n> [!CAUTION]\n> c"
        ));
    }

    #[test]
    fn an_unknown_callout_kind_is_a_plain_quote() {
        assert_eq!(
            html("> [!WHATEVER]\n> x"),
            "<blockquote>\n<p>[!WHATEVER]\nx</p>\n</blockquote>\n"
        );
    }

    // =====================================================================
    // Tables
    // =====================================================================

    #[test]
    fn table_with_every_alignment() {
        insta::assert_snapshot!(html(
            "| L | C | R | D |\n| :-- | :-: | --: | --- |\n| a | *b* | c | d |"
        ));
    }

    #[test]
    fn a_short_body_row_is_padded_with_empty_cells() {
        let out = html("| a | b |\n| --- | --- |\n| 1 |");
        assert!(out.contains("<td>1</td>\n<td></td>\n"), "{out}");
    }

    #[test]
    fn table_cells_escape_their_text() {
        let out = html("| a<b |\n| --- |\n| c&d |");
        assert!(out.contains("<th>a&lt;b</th>"), "{out}");
        assert!(out.contains("<td>c&amp;d</td>"), "{out}");
    }

    // =====================================================================
    // Links
    // =====================================================================

    #[test]
    fn an_external_link_is_left_as_written() {
        let out = emitted("[x](https://e.org/a?b=1&c=2)");
        assert_eq!(
            out.html,
            "<p><a href=\"https://e.org/a?b=1&amp;c=2\">x</a></p>\n"
        );
        assert!(out.assets.is_empty());
    }

    #[test]
    fn a_link_to_a_local_file_is_an_asset_and_goes_through_the_targets() {
        struct Prefixed;
        impl Targets for Prefixed {
            fn note_href(&self, _id: Id, slug: &str) -> String {
                format!("n/{slug}")
            }
            fn asset_href(&self, dest: &str) -> String {
                format!("assets/{dest}")
            }
        }
        let out = emit(
            "[spec](docs/spec.pdf) and ![d](d.png)",
            &[],
            &Prefixed,
            None,
        );
        assert_eq!(
            out.html,
            "<p><a href=\"assets/docs/spec.pdf\">spec</a> and <img src=\"assets/d.png\" alt=\"d\"></p>\n"
        );
        assert_eq!(out.assets, vec!["docs/spec.pdf", "d.png"]);
    }

    #[test]
    fn anchors_mailto_and_protocol_relative_links_are_not_assets() {
        let out = emitted("[a](#top) [m](mailto:x@y.z) [p](//cdn.io/x) [d](data:text/plain,hi)");
        assert!(out.assets.is_empty(), "{:?}", out.assets);
    }

    #[test]
    fn an_asset_is_recorded_once() {
        let out = emitted("![a](x.png) ![b](x.png)");
        assert_eq!(out.assets, vec!["x.png"]);
    }

    #[test]
    fn a_vault_absolute_path_is_a_local_asset() {
        let out = emitted("![logo](/assets/logo.svg)");
        assert_eq!(out.assets, vec!["/assets/logo.svg"]);
    }

    #[test]
    fn autolinks_render_as_anchors() {
        assert_eq!(
            html("see https://a.io, www.b.org, me@c.dev"),
            "<p>see <a href=\"https://a.io\">https://a.io</a>, <a href=\"https://www.b.org\">www.b.org</a>, <a href=\"mailto:me@c.dev\">me@c.dev</a></p>\n"
        );
    }

    #[test]
    fn a_resolved_note_link_shows_the_title_and_points_at_the_sibling_artifact() {
        let input = "[old](01ARZ3NDEKTSV4RRFFQ69G5FAV-old.md)";
        let links = [note_link(
            0..input.len(),
            Some(("New <Title>", "new-title")),
        )];
        assert_eq!(
            emit(input, &links, &SiblingArtifacts, None).html,
            "<p><a class=\"note-link\" href=\"new-title.html\">New &lt;Title&gt;</a></p>\n"
        );
    }

    #[test]
    fn a_dangling_note_link_keeps_its_display_markup() {
        let input = "[**bold** ghost](01ARZ3NDEKTSV4RRFFQ69G5FAV-gone.md)";
        let links = [note_link(0..input.len(), None)];
        assert_eq!(
            emit(input, &links, &SiblingArtifacts, None).html,
            "<p><strong>bold</strong> ghost</p>\n"
        );
    }

    #[test]
    fn a_link_destination_cannot_break_out_of_its_attribute() {
        // Angle brackets let CommonMark accept a destination with spaces
        // and quotes, which is what an attacker-shaped destination needs to
        // reach the attribute at all.
        assert_eq!(
            html("[x](<x.html\" onclick=\"evil()>)"),
            "<p><a href=\"x.html&quot; onclick=&quot;evil()\">x</a></p>\n"
        );
    }

    // =====================================================================
    // Images
    // =====================================================================

    #[test]
    fn a_remote_image_stays_remote_without_a_warning() {
        let out = emitted("![r](https://e.org/i.png)");
        assert_eq!(
            out.html,
            "<p><img src=\"https://e.org/i.png\" alt=\"r\"></p>\n"
        );
        assert!(out.warnings.is_empty());
        assert!(out.assets.is_empty());
    }

    #[test]
    fn an_image_alt_is_flattened_and_escaped() {
        // A tag-shaped `<c>` inside the alt is inline HTML, which the walk
        // drops from alt text; quotes and ampersands are the characters that
        // must survive into the attribute escaped.
        assert_eq!(
            html("![a *b* \"q\" & c](i.png)"),
            "<p><img src=\"i.png\" alt=\"a b &quot;q&quot; &amp; c\"></p>\n"
        );
    }

    // =====================================================================
    // Footnotes
    // =====================================================================

    #[test]
    fn footnotes_are_numbered_in_reference_order_and_listed_at_the_end() {
        insta::assert_snapshot!(html(
            "a[^y] b[^x] c[^y]\n\n[^x]: X with `code`\n\n[^y]: Y\n    - one\n    - two"
        ));
    }

    #[test]
    fn a_definition_referencing_another_footnote_is_patched_in_the_section() {
        let out = html("x[^a]\n\n[^a]: sees[^b]\n\n[^b]: B");
        assert!(!out.contains('\u{0}'), "placeholder leaked: {out:?}");
        assert!(
            out.contains("<li id=\"fn-1\">\n<p>sees<sup class=\"footnote-ref\"><a href=\"#fn-2\" id=\"fnref-2\">2</a></sup></p>\n"),
            "{out}"
        );
    }

    #[test]
    fn an_undefined_reference_is_literal_text() {
        assert_eq!(html("a[^ghost]"), "<p>a[^ghost]</p>\n");
    }

    #[test]
    fn no_footnotes_means_no_section() {
        assert_eq!(html("plain"), "<p>plain</p>\n");
    }

    // =====================================================================
    // Raw HTML
    // =====================================================================

    #[test]
    fn raw_html_passes_through_without_warnings() {
        let out = emitted("<div class=\"x\">block</div>\n\nInline <br> too.");
        assert_eq!(
            out.html,
            "<div class=\"x\">block</div>\n\n<p>Inline <br> too.</p>\n"
        );
        assert!(out.warnings.is_empty());
    }

    // =====================================================================
    // Destination classification
    // =====================================================================

    #[test]
    fn local_destinations_are_those_without_a_scheme() {
        assert!(is_local("diagram.png"));
        assert!(is_local("../assets/logo.svg"));
        assert!(is_local("/assets/logo.svg"));
        assert!(is_local("notes/other.md"));
        assert!(!is_local(""));
        assert!(!is_local("#anchor"));
        assert!(!is_local("//cdn.io/x.png"));
        assert!(!is_local("https://e.org/x"));
        assert!(!is_local("mailto:x@y.z"));
        assert!(!is_local("data:text/plain,hi"));
        assert!(!is_local("git+ssh://host/repo"));
        // A colon that does not terminate a scheme-shaped prefix is a path
        // character.
        assert!(is_local("dir with space:x.png"));
        assert!(is_local("1:2.png"));
    }

    // =====================================================================
    // The whole pipeline over the kitchen-sink fixture
    // =====================================================================

    /// The fixture shared with the CLI contract tests, with its frontmatter
    /// removed the way the note parser does it: the body is what follows the
    /// closing fence.
    fn kitchen_sink_body() -> &'static str {
        const FIXTURE: &str = include_str!("../../../tests/fixtures/kitchen-sink.md");
        let after_opening = &FIXTURE["---\n".len()..];
        let close = after_opening
            .find("\n---\n")
            .expect("the fixture carries frontmatter");
        &after_opening[close + "\n---\n".len()..]
    }

    #[test]
    fn kitchen_sink_pins_the_full_fragment() {
        // Every construct through prepare-equivalent link resolution and the
        // emitter, pinned as one reviewable snapshot: the resolved note link
        // targets a note whose title has changed since the link was written,
        // the dangling one has no target.
        let body = kitchen_sink_body();
        let links: Vec<ResolvedLink> = crate::link::extract(body)
            .into_iter()
            .map(|link| ResolvedLink {
                range: link.range.clone(),
                display: link.display.to_string(),
                id: link.id,
                target: (link.target.starts_with("01BRZ")).then(|| LinkTarget {
                    title: "Current Linked Title".to_string(),
                    slug: "linked".to_string(),
                }),
            })
            .collect();
        assert_eq!(links.len(), 2, "the fixture holds two note links");

        let out = emit(body, &links, &SiblingArtifacts, None);
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert_eq!(out.assets, vec!["notes/other.md", "diagram.png"]);
        assert_eq!(out.languages, vec!["rust"]);
        let outline: Vec<(usize, &str, &str)> = out
            .headings
            .iter()
            .map(|h| (h.level as usize, h.id.as_str(), h.text.as_str()))
            .collect();
        assert_eq!(
            outline,
            vec![
                (2, "table", "Table"),
                (2, "lists", "Lists"),
                (2, "code", "Code"),
                (2, "media-and-html", "Media and HTML"),
            ]
        );
        insta::assert_snapshot!(out.html);
    }
}
