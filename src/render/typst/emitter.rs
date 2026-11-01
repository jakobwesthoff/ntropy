// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The Typst output of the shared Markdown walk: turns a note body into a
//! Typst body fragment (`docs/design/typst-engine.md`, "Element mapping" and
//! "Writer and context model"). The walk itself, and everything that is a
//! property of the Markdown structure rather than of Typst, lives in
//! [`crate::render::markdown`].
//!
//! # Element mapping
//!
//! Block constructs: paragraphs as blank-line separated text, headings as
//! `=` markers ending in a `<ntropy-h-N>` label, fenced and indented code blocks as content-sized raw fences,
//! bullet and ordered lists as `- `/`n. ` markers with continuation lines
//! indented under the marker, block quotes as `#quote`, GFM callouts as
//! `#callout(kind:)`, tables as `#table`, thematic breaks as `#line`.
//!
//! Inline constructs, all emitted as Typst function calls so that no
//! user-derived character is ever load-bearing markup. Every call the emitter
//! leaves in running text ends in `;`, which the parser consumes: without it a
//! `(` in the note's text right after the call reads as more arguments, and a
//! `;` there disappears. A call that emitter syntax follows directly
//! (`#notelink` inside the link body, `#footnote` before its label) has none,
//! and `#task` ends in a space.
//!
//! - Emphasis, strong, and strikethrough wrap their content in `#emph[…]`,
//!   `#strong[…]`, and `#strike[…]`; the wrappers nest.
//! - Inline code becomes `#raw("…")` with string-literal escaping.
//! - Soft breaks map to a space and hard breaks to `#linebreak()`.
//! - A resolved note link renders as `#link("<target-slug>.pdf")[#notelink[Title]]`,
//!   so the artifact carries a clickable target next to itself (ADR 0044).
//!   A link to a fragment of the note itself, or a note link to the note itself
//!   that carries a heading anchor (ADR 0088), renders as
//!   `#link(<ntropy-h-N>)[label]`, naming the label of the heading with that
//!   id, and as plain text with a warning when no heading has it. Every other
//!   link, autolinks included, renders as `#link("target")[label]`.
//! - A local image becomes `#image("path", alt: "text")`, rewritten to a
//!   root-absolute path under the note's directory (ADR 0045); a remote
//!   `http(s)` image
//!   cannot be embedded by the offline compiler, so it degrades to
//!   `#link("url")[alt-or-url]` and raises a [`Warning`].
//! - A footnote is drawn at its first reference in the text as
//!   `#[#footnote[…]<ntropy-fn-N>];`, and every later reference, inside a
//!   footnote's content included, is `#footnote(<ntropy-fn-N>);`, which shows
//!   the same number.
//! - Raw HTML, blocks and inline fragments alike, is dropped and raises a
//!   [`Warning`] naming the dropped fragment.
//!
//! Every character of user text passes through one of the [`TypstWriter`]'s
//! escaped channels; already-rendered children are spliced back through the
//! unescaped `syntax` channel.

use std::collections::HashMap;

use super::writer::{self, TypstWriter};
use crate::id::Id;
use crate::render::ResolvedLink;
use crate::render::markdown::{
    self, Alignment, BlockQuoteKind, Heading, InlineStyle, List, NoteLabel, Output, Table, Warning,
    note_label,
};

/// The extension a resolved note link targets: the artifact a default
/// `ntropy render` of the target note produces (ADR 0044).
const NOTE_LINK_EXTENSION: &str = "pdf";

// =========================================================
// Public entry
// =========================================================

/// Convert a note body to a Typst body fragment, resolving note links against
/// `links` (ADR 0028).
///
/// The returned string is the converted body alone; document assembly
/// (prelude, template application) lives in the engine layer, which also
/// forwards the returned warnings to the host.
///
/// The document's `note` function receives `title`, and the built-in one
/// draws it above the body. A body whose first block the page shows is a
/// level-one heading with exactly that plain text, image alt text included,
/// would repeat it, so the walk drops that heading. Raw HTML the output drops
/// does not count as a shown block. A theme whose `note` does not draw
/// `title` therefore shows no title. The HTML output applies the same rule
/// ([`crate::render::html::emitter::emit`]).
pub fn emit(
    body: &str,
    links: &[ResolvedLink],
    asset_base: &str,
    title: Option<&str>,
) -> (String, Vec<Warning>) {
    emit_for_note(body, links, asset_base, title, None)
}

/// [`emit`] for the note with id `own_id`: a note link to that note with a
/// heading anchor links to the heading inside the document.
pub fn emit_for_note(
    body: &str,
    links: &[ResolvedLink],
    asset_base: &str,
    title: Option<&str>,
    own_id: Option<Id>,
) -> (String, Vec<Warning>) {
    let mut output = TypstOutput {
        asset_base,
        title: title.map(|title| title.trim().to_string()),
        own_id,
        heading_labels: HashMap::new(),
    };
    markdown::walk(body, links, &mut output)
}

/// The label of the `number`th heading drawn. Like the footnote labels, the
/// `ntropy-h-` prefix belongs to ntropy.
fn heading_label(number: usize) -> String {
    format!("ntropy-h-{number}")
}

/// The label of footnote `number`. The `ntropy-fn-` prefix belongs to ntropy,
/// so a theme's own labels never collide with it.
fn footnote_label(number: usize) -> String {
    format!("ntropy-fn-{number}")
}

// =========================================================
// The Typst output
// =========================================================

struct TypstOutput<'a> {
    /// The note's own directory as a compile-root-absolute path, e.g.
    /// `/all-notes`. Relative image paths are resolved against it so they
    /// survive a compile whose root is the vault rather than the note's
    /// directory (ADR 0045).
    asset_base: &'a str,
    /// The title a leading level-one heading must equal to be dropped.
    title: Option<String>,
    /// The id of the note being emitted, `None` when it is not known.
    own_id: Option<Id>,
    /// The label number of each heading drawn so far, by heading id, in the
    /// order the headings were handed over. A link to a heading names the
    /// label by this number: the id itself can hold characters, `²` or `½`
    /// among them, that Typst rejects in a label.
    heading_labels: HashMap<String, usize>,
}

impl Output for TypstOutput<'_> {
    fn text(&mut self, text: &str) -> String {
        let mut writer = TypstWriter::new();
        writer.markup_text(text);
        writer.finish()
    }

    fn inline_code(&mut self, code: &str) -> String {
        let mut writer = TypstWriter::new();
        writer.syntax("#raw(\"");
        writer.string_literal(code);
        writer.syntax("\");");
        writer.finish()
    }

    fn soft_break(&mut self) -> String {
        " ".to_string()
    }

    fn hard_break(&mut self) -> String {
        "#linebreak();".to_string()
    }

    /// The prelude defines `task`, which draws one identical checkbox for both
    /// states, so checked and unchecked always match optically regardless of
    /// font glyph coverage.
    fn task_marker(&mut self, checked: bool) -> String {
        if checked {
            "#task(done: true) ".to_string()
        } else {
            "#task(done: false) ".to_string()
        }
    }

    fn styled(&mut self, style: InlineStyle, inner: &str) -> String {
        let call = match style {
            InlineStyle::Emph => "emph",
            InlineStyle::Strong => "strong",
            InlineStyle::Strike => "strike",
        };
        format!("#{call}[{inner}];")
    }

    fn link(&mut self, dest: &str, inner: &str) -> String {
        wrap_link(dest, inner)
    }

    /// `#link("<slug>.pdf")[#notelink[Title]]`, with `slug` string-literal
    /// escaped and `title` escaped as markup text. A link with a heading
    /// anchor shows the author's text instead of the title and drops the
    /// anchor, since the heading lives in the target's own PDF, which this
    /// link cannot address (ADR 0086).
    ///
    /// The target is the artifact a default `ntropy render` of the target note
    /// produces in the same directory, so a set of notes rendered together
    /// cross-navigates (ADR 0044). The extension is always `pdf`, never the
    /// extension of the format being produced: the `typst` artifact is the
    /// source of a PDF, and both formats emit identical bytes by design.
    /// `notelink` is defined by the prelude, so a theme can style note links
    /// distinctly from ordinary emphasis.
    fn note_link(
        &mut self,
        _id: Id,
        title: &str,
        slug: &str,
        fragment: Option<&str>,
        inner: &str,
    ) -> String {
        let mut writer = TypstWriter::new();
        writer.syntax("#link(\"");
        writer.string_literal(&format!("{slug}.{NOTE_LINK_EXTENSION}"));
        writer.syntax("\")[#notelink[");
        match note_label(title, fragment, inner) {
            NoteLabel::Title(title) => writer.markup_text(title),
            NoteLabel::Inner(inner) => writer.raw(inner),
        }
        writer.syntax("]];");
        writer.finish()
    }

    /// A local path becomes `#image("path", alt: "text")`, with the alt text
    /// left out when it is blank; a remote one cannot be embedded offline and
    /// degrades to a link plus a warning.
    fn image(&mut self, dest: &str, alt: &str, warnings: &mut Vec<Warning>) -> String {
        let lowered = dest.to_ascii_lowercase();
        let remote = lowered.starts_with("http://") || lowered.starts_with("https://");

        let mut writer = TypstWriter::new();
        if remote {
            // The label falls back to the URL when the alt is empty, so the
            // degraded link is never blank.
            let label = if alt.is_empty() { dest } else { alt };
            writer.syntax("#link(\"");
            writer.string_literal(dest);
            writer.syntax("\")[");
            writer.markup_text(label);
            writer.syntax("];");
            warnings.push(Warning::new(format!(
                "remote image cannot be embedded; linked instead: {dest}"
            )));
        } else {
            // A fragment selects nothing in an embedded image, and typst
            // would look for a file with the `#` in its name.
            let path = dest.split_once('#').map_or(dest, |(path, _)| path);
            writer.syntax("#image(\"");
            writer.string_literal(&resolve_asset(self.asset_base, path));
            writer.syntax("\"");
            // Character references can put control characters into the alt;
            // a newline would be split by a list item's continuation indent
            // inside the string. Typst records a blank alt as a present but
            // empty description, so only text that remains gets written.
            let alt: String = alt
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            let alt = alt.trim();
            if !alt.is_empty() {
                writer.syntax(", alt: \"");
                writer.string_literal(alt);
                writer.syntax("\"");
            }
            writer.syntax(");");
        }
        writer.finish()
    }

    /// The footnote, drawn at its first place in the text and labelled so
    /// that later references can point at it. The `#[...]` wrapper keeps the
    /// label on the footnote when the reference sits in a heading, where a
    /// bare label after the call would attach to the heading. The closing
    /// `;` ends the expression, so a `(` that follows in the note's text is
    /// text and not a call on the footnote.
    fn footnote(&mut self, number: usize, content: &str) -> String {
        format!(
            "#[#footnote[{}]<{}>];",
            content.trim_end_matches('\n'),
            footnote_label(number)
        )
    }

    /// Typst shows the number of the labelled footnote and draws nothing
    /// else, so the reference may sit inside the footnote's own content.
    fn footnote_again(&mut self, number: usize) -> String {
        format!("#footnote(<{}>);", footnote_label(number))
    }

    fn inline_html(&mut self, html: &str, warnings: &mut Vec<Warning>) -> Option<String> {
        warnings.push(Warning::new(format!(
            "dropped raw HTML: {}",
            truncate_fragment(html)
        )));
        None
    }

    fn paragraph(&mut self, body: &str) -> String {
        format!("{body}\n")
    }

    fn dropped_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The heading ends in a label (`<ntropy-h-N>`), so a link to it, from a
    /// note link to its fragment, has something to name. The label follows
    /// the text after a space, which attaches it to the heading whatever
    /// call the text ends in.
    fn heading(&mut self, heading: &Heading, body: &str) -> String {
        let marker = "=".repeat(heading.level as usize);
        let number = self.heading_labels.len() + 1;
        self.heading_labels.insert(heading.id.clone(), number);
        format!("{marker} {body} <{}>\n", heading_label(number))
    }

    /// A link to a heading of the note: `#link(<ntropy-h-N>)[...];`. A
    /// fragment that names no heading is the label alone and a warning, since
    /// a label that does not exist fails the compile. `#` and `#top` name the
    /// top of the page when no heading has that id, as in a browser. A note
    /// link to the note itself keeps `notelink` around its label, so a theme
    /// styles it like every other note link.
    fn anchor_link(
        &mut self,
        fragment: &str,
        heading: Option<&str>,
        inner: &str,
        note: bool,
        warnings: &mut Vec<Warning>,
    ) -> String {
        let label = if note {
            format!("#notelink[{inner}]")
        } else {
            inner.to_string()
        };
        if let Some(number) = heading.and_then(|id| self.heading_labels.get(id)) {
            return format!("#link(<{}>)[{label}];", heading_label(*number));
        }
        if fragment.is_empty() || fragment.eq_ignore_ascii_case("top") {
            return format!("#link((page: 1, x: 0pt, y: 0pt))[{label}];");
        }
        warnings.push(Warning::new(format!(
            "link to `#{fragment}` names no heading of the note; shown as text"
        )));
        if note { format!("{label};") } else { label }
    }

    fn own_note(&self) -> Option<Id> {
        self.own_id
    }

    /// The ids `fn-N` and `fnref-N` stay free of headings so that a heading
    /// id is the same here as in the HTML output of the same note.
    fn reserves_id(&self, id: &str, footnotes: usize) -> bool {
        markdown::is_numbered_footnote_id(id, footnotes)
    }

    fn quote(&mut self, kind: Option<BlockQuoteKind>, body: &str) -> String {
        wrap_quote(kind, body)
    }

    fn code_block(&mut self, info: Option<&str>, content: &str) -> String {
        let language = info.and_then(language_tag);
        render_code_block(language.as_deref(), content)
    }

    /// Items get `- ` or `n. ` markers with continuation lines indented to
    /// the marker's width; a loose list separates its items with blank lines.
    fn list(&mut self, list: &List) -> String {
        let mut next = list.start.unwrap_or(0);
        let items: Vec<String> = list
            .items
            .iter()
            .map(|body| {
                let body = body.trim_end_matches('\n');
                let marker = match list.start {
                    // `pulldown-cmark` reports the real start number, so
                    // `6.` `7.` `8.` survive without a Typst `start:` argument.
                    Some(_) => {
                        let marker = format!("{next}. ");
                        next += 1;
                        marker
                    }
                    None => "- ".to_string(),
                };
                indent_continuation(body, &marker)
            })
            .collect();
        let joiner = if list.loose { "\n\n" } else { "\n" };
        format!("{}\n", items.join(joiner))
    }

    fn table(&mut self, table: &Table) -> String {
        render_table(table)
    }

    fn rule(&mut self) -> String {
        "#line(length: 100%)\n".to_string()
    }

    fn html_block(&mut self, html: &str, warnings: &mut Vec<Warning>) -> Option<String> {
        warnings.push(Warning::new(format!(
            "dropped raw HTML: {}",
            truncate_fragment(html)
        )));
        None
    }
}

// =========================================================
// Block rendering
// =========================================================

/// Wrap a container's child blocks as a block quote or a callout. The kind is
/// present only for GFM callouts; a plain quote carries `None`.
fn wrap_quote(kind: Option<BlockQuoteKind>, children: &str) -> String {
    let mut writer = TypstWriter::new();
    match kind {
        None => writer.syntax("#quote(block: true)[\n"),
        Some(kind) => {
            writer.syntax("#callout(kind: \"");
            writer.string_literal(callout_kind(kind));
            writer.syntax("\")[\n");
        }
    }
    writer.syntax(children);
    writer.syntax("]\n");
    writer.finish()
}

/// The lowercase kind tag a callout carries into the artifact. The prelude's
/// `callout` function dispatches on this string.
fn callout_kind(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "note",
        BlockQuoteKind::Tip => "tip",
        BlockQuoteKind::Important => "important",
        BlockQuoteKind::Warning => "warning",
        BlockQuoteKind::Caution => "caution",
    }
}

/// Extract the Typst language tag from a Markdown fence info string.
///
/// The info string is user-derived and the tag is spliced after the opening
/// fence unescaped, where Typst reads everything up to the first whitespace
/// as the language. Only the first whitespace-delimited token qualifies, and
/// only when it is identifier-shaped (alphanumeric, `-`, `_`); anything else
/// would change how Typst parses the raw block's first line, so it yields no
/// tag rather than a corrupted one.
fn language_tag(info: &str) -> Option<String> {
    let token = info.split_whitespace().next()?;
    if token
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        Some(token.to_string())
    } else {
        None
    }
}

/// Emit a Typst raw block. The fence is sized from the content so the content
/// cannot close it; the language tag, when present, follows the opening fence
/// directly; the content passes through verbatim.
fn render_code_block(language: Option<&str>, content: &str) -> String {
    let fence = writer::fence(content);
    let mut writer = TypstWriter::new();
    writer.syntax(&fence);
    if let Some(language) = language {
        writer.syntax(language);
    }
    writer.syntax("\n");
    writer.raw(content);
    // Guarantee the closing fence starts its own line even when the content
    // does not end in a newline (an indented block's final line can lack one).
    if !content.is_empty() && !content.ends_with('\n') {
        writer.syntax("\n");
    }
    writer.syntax(&fence);
    writer.syntax("\n");
    writer.finish()
}

/// Render a `#table(...)`. The header fixes the column count and per-column
/// alignment.
fn render_table(table: &Table) -> String {
    let columns = table.header.len();

    let mut writer = TypstWriter::new();
    writer.syntax(&format!("#table(\ncolumns: {columns},\n"));

    let alignments: Vec<&str> = (0..columns)
        .map(|column| match table.alignments.get(column) {
            Some(Alignment::Left) => "left",
            Some(Alignment::Center) => "center",
            Some(Alignment::Right) => "right",
            // No explicit alignment (or a missing entry) leaves the column to
            // Typst's default.
            _ => "auto",
        })
        .collect();
    writer.syntax(&format!("align: ({},),\n", alignments.join(", ")));

    writer.syntax(&format!("table.header({},),\n", join_cells(&table.header)));

    for row in &table.rows {
        writer.syntax(&format!("{},\n", join_cells(row)));
    }

    writer.syntax(")\n");
    writer.finish()
}

/// Render each cell as a `[...]` content block and join them for one row. The
/// cell strings already carry their escaped markup.
fn join_cells(cells: &[String]) -> String {
    cells
        .iter()
        .map(|cell| format!("[{cell}]"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Prefix a list item's body with its marker and indent every continuation
/// line to the marker's width, so Typst keeps nested lists and continuation
/// blocks attached to the item.
fn indent_continuation(body: &str, marker: &str) -> String {
    let indent = " ".repeat(marker.chars().count());
    let mut out = String::new();
    for (line_number, line) in body.split('\n').enumerate() {
        if line_number == 0 {
            out.push_str(marker);
            out.push_str(line);
        } else {
            out.push('\n');
            // Blank separator lines stay blank; trailing indentation on them
            // would be noise Typst does not need.
            if !line.is_empty() {
                out.push_str(&indent);
                out.push_str(line);
            }
        }
    }
    out
}

// =========================================================
// Inline rendering
// =========================================================

/// Resolve a local image path against the note's directory, as a path absolute
/// within the compile root.
///
/// The compiler's root is the vault, not the note's directory, so a document
/// piped to it behaves as if it sat at the root: `diagram.png` beside a note
/// would be looked for at the vault root and not found (ADR 0045). Joining the
/// note's own root-absolute directory onto the path restores what the author
/// meant, and lets a note reach a vault asset with `../assets/logo.svg` at the
/// same time.
///
/// A path that is already root-absolute is the author addressing the vault
/// directly; its `.` and `..` parts are resolved the same way, without the
/// note's directory. A path that climbs out of the root altogether, from the
/// note or from the root, cannot be expressed as a root-absolute path and
/// passes through as written; the compiler rejects it by name rather than by
/// an invented substitute.
fn resolve_asset(base: &str, dest: &str) -> String {
    // Textual resolution against the root-absolute `base` is what Typst's own
    // root-relative lookup does; a path that escapes the root stays as
    // written for the compiler to reject by name.
    markdown::resolve_root_relative(base, dest).unwrap_or_else(|| dest.to_string())
}

/// A `#link("dest")[inner]` call: `dest` string-literal escaped, `inner`
/// already-escaped markup.
fn wrap_link(dest: &str, inner: &str) -> String {
    let mut writer = TypstWriter::new();
    writer.syntax("#link(\"");
    writer.string_literal(dest);
    writer.syntax("\")[");
    writer.syntax(inner);
    writer.syntax("];");
    writer.finish()
}

/// Trim and shorten a dropped fragment for a warning message, keeping the
/// message bounded regardless of the fragment's size.
fn truncate_fragment(fragment: &str) -> String {
    const MAX: usize = 60;
    let trimmed = fragment.trim();
    let mut out: String = trimmed.chars().take(MAX).collect();
    if trimmed.chars().count() > MAX {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Range;

    use crate::render::LinkTarget;

    use typst_syntax::{SyntaxKind, SyntaxNode};

    /// Emit a body with no note-link table, keeping the string alone. Most
    /// tests exercise constructs the note-link table does not touch.
    /// The note directory every emitter test resolves assets against: the
    /// standard vault layout, where notes live in `all-notes/`.
    const ASSET_BASE: &str = "/all-notes";

    fn body(input: &str) -> String {
        emit(input, &[], ASSET_BASE, None).0
    }

    /// Emit a body for a note with the given title.
    fn body_titled(input: &str, title: &str) -> String {
        emit(input, &[], ASSET_BASE, Some(title)).0
    }

    /// Emit and keep only the warnings, for the raw-HTML and remote-image
    /// cases whose contract is the warning rather than the body.
    fn warnings(input: &str) -> Vec<Warning> {
        emit(input, &[], ASSET_BASE, None).1
    }

    /// Build a note-link table entry from an optional `(title, slug)` target.
    /// The range must equal the Link event's byte span in `input` for the
    /// emitter to treat it as a note link.
    fn note_link(range: Range<usize>, target: Option<(&str, &str)>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: crate::id::Id::from_timestamp_ms(0),
            anchored: false,
            target: target.map(|(title, slug)| LinkTarget {
                title: title.to_string(),
                slug: slug.to_string(),
            }),
        }
    }

    // =====================================================================
    // Paragraphs
    // =====================================================================

    #[test]
    fn empty_body_emits_nothing() {
        assert_eq!(body(""), "");
    }

    #[test]
    fn single_paragraph_is_escaped_text_with_a_trailing_newline() {
        assert_eq!(body("Just some prose."), "Just some prose\\.\n");
    }

    #[test]
    fn adjacent_paragraphs_are_separated_by_a_blank_line() {
        insta::assert_snapshot!(body("First paragraph.\n\nSecond paragraph."));
    }

    #[test]
    fn soft_wrapped_lines_join_with_a_space() {
        // A soft break inside one paragraph is a single space; the paragraph
        // stays one line of output.
        assert_eq!(body("one\ntwo\nthree"), "one two three\n");
    }

    #[test]
    fn hard_break_becomes_a_linebreak_call() {
        // Two trailing spaces force a hard break.
        assert_eq!(body("one  \ntwo"), "one#linebreak();two\n");
    }

    // =====================================================================
    // Headings
    // =====================================================================

    #[test]
    fn headings_of_every_level_carry_escaped_markup_active_text() {
        // Each level's text holds characters Typst markup reacts to, proving
        // the heading text channel escapes while the `=` prefix does not.
        let input = "\
# Heading *one* level
## Heading _two_ level
### Heading `three` level
#### Heading [four] level
##### Heading ~five = plus + level
###### Heading #six level";
        insta::assert_snapshot!(body(input));
    }

    // =====================================================================
    // Code blocks
    // =====================================================================

    #[test]
    fn fenced_code_with_a_language_tag() {
        insta::assert_snapshot!(body("```rust\nlet x = 1;\n```"));
    }

    #[test]
    fn fenced_code_without_a_language_tag() {
        insta::assert_snapshot!(body("```\nplain code\n```"));
    }

    #[test]
    fn fence_info_string_keeps_only_the_first_token() {
        // GFM allows arbitrary words after the language (` ```rust ignore `);
        // Typst reads everything up to whitespace as the tag, so only the
        // first token survives.
        assert_eq!(
            body("```rust ignore\nlet x = 1;\n```"),
            "```rust\nlet x = 1;\n```\n"
        );
    }

    #[test]
    fn fence_info_string_with_typst_active_characters_yields_no_tag() {
        // An info string that is not identifier-shaped would change how Typst
        // parses the raw block's first line; it is dropped entirely.
        assert_eq!(body("```a]b#c\ncode\n```"), "```\ncode\n```\n");
    }

    #[test]
    fn code_containing_a_triple_backtick_grows_the_fence() {
        // The content documents a Markdown fence, so the emitted fence must be
        // one backtick longer than the run inside it.
        insta::assert_snapshot!(body("````\na ``` fence inside\n````"));
    }

    #[test]
    fn indented_code_block() {
        insta::assert_snapshot!(body("    indented code\n    second line"));
    }

    #[test]
    fn empty_code_block() {
        insta::assert_snapshot!(body("```\n```"));
    }

    // =====================================================================
    // Bullet lists
    // =====================================================================

    #[test]
    fn flat_bullet_list() {
        assert_eq!(body("- a\n- b\n- c"), "- a\n- b\n- c\n");
    }

    #[test]
    fn bullet_list_nested_three_deep() {
        insta::assert_snapshot!(body("- outer\n  - middle\n    - inner"));
    }

    #[test]
    fn adjacent_bullet_lists_separated_by_a_paragraph() {
        // A paragraph between two lists keeps them as two separate blocks.
        insta::assert_snapshot!(body("- a\n- b\n\ntext between\n\n- c\n- d"));
    }

    // =====================================================================
    // Ordered lists
    // =====================================================================

    #[test]
    fn ordered_list_starting_at_one() {
        assert_eq!(body("1. a\n2. b\n3. c"), "1. a\n2. b\n3. c\n");
    }

    #[test]
    fn ordered_list_starting_at_six_keeps_the_real_numbers() {
        assert_eq!(
            body("6. six\n7. seven\n8. eight"),
            "6. six\n7. seven\n8. eight\n"
        );
    }

    #[test]
    fn ordered_and_bullet_lists_nested_together() {
        insta::assert_snapshot!(body("1. first\n   - bullet a\n   - bullet b\n2. second"));
    }

    // =====================================================================
    // Task lists
    // =====================================================================

    #[test]
    fn task_list_checked_and_unchecked() {
        insta::assert_snapshot!(body("- [ ] todo\n- [x] done"));
    }

    // =====================================================================
    // List tightness
    // =====================================================================

    #[test]
    fn tight_list_items_sit_on_adjacent_lines() {
        assert_eq!(body("- a\n- b"), "- a\n- b\n");
    }

    #[test]
    fn loose_list_items_are_separated_by_blank_lines() {
        insta::assert_snapshot!(body("- a\n\n- b"));
    }

    #[test]
    fn loose_item_with_a_continuation_paragraph_indents_it() {
        insta::assert_snapshot!(body(
            "- First paragraph.\n\n  Second paragraph.\n- Next item"
        ));
    }

    #[test]
    fn list_item_with_a_nested_code_block_indents_it() {
        insta::assert_snapshot!(body("- item\n\n  ```\n  code\n  ```"));
    }

    // =====================================================================
    // Block quotes
    // =====================================================================

    #[test]
    fn quote_with_multiple_paragraphs() {
        insta::assert_snapshot!(body("> First.\n>\n> Second."));
    }

    // =====================================================================
    // Callouts
    // =====================================================================

    #[test]
    fn all_five_callout_kinds() {
        let input = "\
> [!NOTE]
> a note

> [!TIP]
> a tip

> [!IMPORTANT]
> an important

> [!WARNING]
> a warning

> [!CAUTION]
> a caution";
        insta::assert_snapshot!(body(input));
    }

    #[test]
    fn callout_kind_is_lowercased() {
        // GFM matches the marker case-insensitively; the emitted kind is
        // always lowercase regardless of how the note wrote it.
        assert!(body("> [!NoTe]\n> body").contains("#callout(kind: \"note\")["));
    }

    #[test]
    fn unknown_callout_kind_falls_back_to_a_plain_quote() {
        // An unrecognized marker is not a callout: it renders as an ordinary
        // quote and the `[!FOO]` text survives escaped in the body.
        insta::assert_snapshot!(body("> [!FOO]\n> body"));
    }

    // =====================================================================
    // Tables
    // =====================================================================

    #[test]
    fn basic_table() {
        insta::assert_snapshot!(body("| H1 | H2 |\n| --- | --- |\n| a | b |\n| c | d |"));
    }

    #[test]
    fn table_with_every_alignment() {
        insta::assert_snapshot!(body(
            "| L | C | R |\n| :--- | :---: | ---: |\n| a | b | c |"
        ));
    }

    #[test]
    fn table_with_default_alignment() {
        // A header separator with no colons leaves the column alignment unset,
        // which maps to Typst's `auto`.
        insta::assert_snapshot!(body("| H1 | H2 |\n| --- | --- |\n| a | b |"));
    }

    #[test]
    fn table_body_row_shorter_than_the_header_is_padded() {
        // The second body row omits its last cell; the emitter pads it so
        // every row matches the header's column count.
        insta::assert_snapshot!(body(
            "| H1 | H2 | H3 |\n| --- | --- | --- |\n| a | b | c |\n| d |"
        ));
    }

    #[test]
    fn table_cells_escape_markup_active_text() {
        insta::assert_snapshot!(body("| Name | Note |\n| --- | --- |\n| a*b | c_d |"));
    }

    // =====================================================================
    // Thematic breaks
    // =====================================================================

    #[test]
    fn thematic_break_between_paragraphs() {
        insta::assert_snapshot!(body("before\n\n---\n\nafter"));
    }

    // =====================================================================
    // Emphasis, strong, strikethrough
    // =====================================================================

    #[test]
    fn emphasis_strong_and_strike_become_function_calls() {
        assert_eq!(
            body("*a* **b** ~~c~~"),
            "#emph[a]; #strong[b]; #strike[c];\n"
        );
    }

    #[test]
    fn underscore_emphasis_matches_asterisk_emphasis() {
        assert_eq!(body("_a_ __b__"), "#emph[a]; #strong[b];\n");
    }

    #[test]
    fn nested_span_styling_nests_the_calls() {
        // Strong wrapping emphasis wrapping strikethrough proves the frames
        // stack without the delimiters ever reaching the output.
        assert_eq!(
            body("**bold _italic ~~struck~~_**"),
            "#strong[bold #emph[italic #strike[struck];];];\n"
        );
    }

    #[test]
    fn adjacent_styled_words_stay_separate_calls() {
        // Strong immediately followed by strikethrough, no separator: two
        // independent calls with nothing between them.
        assert_eq!(body("**a**~~b~~"), "#strong[a];#strike[b];\n");
    }

    #[test]
    fn styled_text_still_escapes_markup_active_characters() {
        // The inner text passes through the markup channel, so a `#` inside an
        // emphasis is escaped just as it is in plain prose.
        assert_eq!(body("*a#b*"), "#emph[a\\#b];\n");
    }

    // =====================================================================
    // Inline code
    // =====================================================================

    #[test]
    fn inline_code_becomes_raw_with_string_escaping() {
        assert_eq!(body("`plain`"), "#raw(\"plain\");\n");
    }

    #[test]
    fn inline_code_escapes_quotes_and_backslashes() {
        // The two string-literal escapes apply; the markup set does not.
        assert_eq!(body(r#"`a "b" \c`"#), "#raw(\"a \\\"b\\\" \\\\c\");\n");
    }

    #[test]
    fn inline_code_with_backticks_inside() {
        // A double-backtick span carries a literal backtick, which needs no
        // escaping inside a Typst string.
        assert_eq!(body("`` a`b ``"), "#raw(\"a`b\");\n");
    }

    // =====================================================================
    // Ordinary links
    // =====================================================================

    #[test]
    fn external_link() {
        assert_eq!(
            body("[label](https://example.com)"),
            "#link(\"https://example.com\")[label];\n"
        );
    }

    #[test]
    fn relative_and_mailto_links_pass_the_target_verbatim() {
        assert_eq!(body("[a](./notes/b.md)"), "#link(\"./notes/b.md\")[a];\n");
        assert_eq!(
            body("[a](mailto:x@example.org)"),
            "#link(\"mailto:x@example.org\")[a];\n"
        );
    }

    #[test]
    fn link_destination_with_a_quote_is_string_escaped() {
        assert_eq!(
            body(r#"[a](https://example.com/?q="x")"#),
            "#link(\"https://example.com/?q=\\\"x\\\"\")[a];\n"
        );
    }

    #[test]
    fn angle_bracket_url_autolink_is_an_ordinary_link() {
        // The label is the URL passed through the markup channel, so its
        // punctuation is escaped.
        assert_eq!(
            body("<https://example.com>"),
            "#link(\"https://example.com\")[https\\:\\/\\/example\\.com];\n"
        );
    }

    #[test]
    fn angle_bracket_email_autolink_gains_the_mailto_scheme() {
        // pulldown-cmark hands an email autolink through with the bare address
        // as the destination; a PDF link needs the `mailto:` scheme to act on
        // it, so the emitter adds it while the label stays as written.
        assert_eq!(
            body("<foo@example.org>"),
            "#link(\"mailto:foo@example.org\")[foo\\@example\\.org];\n"
        );
    }

    #[test]
    fn link_label_carrying_markup_keeps_the_markup() {
        assert_eq!(body("[*a* b](u)"), "#link(\"u\")[#emph[a]; b];\n");
    }

    #[test]
    fn link_inside_emphasis_and_emphasis_inside_link() {
        assert_eq!(body("*[a](u)*"), "#emph[#link(\"u\")[a];];\n");
        assert_eq!(body("[**a**](u)"), "#link(\"u\")[#strong[a];];\n");
    }

    // =====================================================================
    // Note links
    // =====================================================================

    #[test]
    fn resolved_note_link_becomes_a_link_to_the_targets_pdf() {
        // `[my note](abc123)` spans bytes 4..21; a matching resolved entry
        // replaces the whole link with its target's current title through the
        // prelude-defined `notelink` function, wrapped in a `#link` at the
        // artifact a default render of the target produces (ADR 0044).
        let input = "See [my note](abc123) end.";
        let links = [note_link(4..21, Some(("Target Title", "target-title")))];
        assert_eq!(
            emit(input, &links, ASSET_BASE, None).0,
            "See #link(\"target-title.pdf\")[#notelink[Target Title]]; end\\.\n"
        );
    }

    #[test]
    fn resolved_note_link_targets_the_slug_not_the_title() {
        // The slug is the artifact's stem and the title is only what the
        // reader sees; a title that would slugify differently must not leak
        // into the target.
        let input = "[x](id)";
        let links = [note_link(0..7, Some(("A Renamed Note", "old-slug")))];
        assert_eq!(
            emit(input, &links, ASSET_BASE, None).0,
            "#link(\"old-slug.pdf\")[#notelink[A Renamed Note]];\n"
        );
    }

    /// The link table `prepare` builds for `body`, every note link resolving
    /// to a note titled `Target Title`.
    fn resolved_links(body: &str) -> Vec<ResolvedLink> {
        crate::link::extract(body)
            .into_iter()
            .map(|link| ResolvedLink {
                range: link.range,
                display: link.display.to_string(),
                id: link.id,
                anchored: link.anchored,
                target: Some(LinkTarget {
                    title: "Target Title".to_string(),
                    slug: "target-title".to_string(),
                }),
            })
            .collect()
    }

    /// Emit `input` with every note link resolving and no note id known.
    fn body_with_links(input: &str) -> String {
        emit(input, &resolved_links(input), ASSET_BASE, None).0
    }

    #[test]
    fn an_anchored_note_link_keeps_the_authors_text_and_drops_the_anchor() {
        // The heading lives in the target's own PDF, which a link cannot
        // address, so the link opens the target's artifact; the author's text
        // still says what the link is about.
        let input = "[the **tmux** part](01ARZ3NDEKTSV4RRFFQ69G5FAV-old.md#inside-tmux)";
        assert_eq!(
            emit(input, &resolved_links(input), ASSET_BASE, None).0,
            "#link(\"target-title.pdf\")[#notelink[the #strong[tmux]; part]];\n"
        );
    }

    #[test]
    fn an_anchored_note_link_without_text_shows_the_title() {
        let input = "[ ](01ARZ3NDEKTSV4RRFFQ69G5FAV-old.md#inside-tmux)";
        assert_eq!(
            emit(input, &resolved_links(input), ASSET_BASE, None).0,
            "#link(\"target-title.pdf\")[#notelink[Target Title]];\n"
        );
    }

    #[test]
    fn resolved_note_link_discards_inner_markup_label() {
        // The display text carries markup, but a resolved note link shows the
        // target's title instead, so the inner `#strong` never appears.
        let input = "[**bold** display](id)";
        let links = [note_link(0..22, Some(("The Title", "the-title")))];
        assert_eq!(
            emit(input, &links, ASSET_BASE, None).0,
            "#link(\"the-title.pdf\")[#notelink[The Title]];\n"
        );
    }

    #[test]
    fn resolved_note_link_title_escapes_markup_active_characters() {
        let input = "[x](id)";
        let links = [note_link(0..7, Some(("a*b [c] #d", "a-b-c-d")))];
        assert_eq!(
            emit(input, &links, ASSET_BASE, None).0,
            "#link(\"a-b-c-d.pdf\")[#notelink[a\\*b \\[c\\] \\#d]];\n"
        );
    }

    #[test]
    fn unresolved_note_link_re_emits_its_markup_label() {
        // A dangling note link drops the wrapper and keeps the display markup.
        let input = "[*display* text](id)";
        let links = [note_link(0..20, None)];
        assert_eq!(
            emit(input, &links, ASSET_BASE, None).0,
            "#emph[display]; text\n"
        );
    }

    #[test]
    fn resolved_link_whose_range_matches_no_event_is_ignored() {
        // The note text already renders literally; a stray table entry that
        // matches no Link event changes nothing.
        let input = "just prose";
        let links = [note_link(0..4, Some(("Title", "title")))];
        assert_eq!(emit(input, &links, ASSET_BASE, None).0, "just prose\n");
    }

    #[test]
    fn link_event_without_a_matching_entry_is_an_ordinary_link() {
        // Same input as the resolved case, but an empty table: the link falls
        // back to an ordinary `#link`, not a note link.
        let input = "See [my note](abc123) end.";
        assert_eq!(body(input), "See #link(\"abc123\")[my note]; end\\.\n");
    }

    // =====================================================================
    // Images
    // =====================================================================

    #[test]
    fn local_image_becomes_an_image_call_with_its_alt() {
        assert_eq!(
            body("![alt text](pics/x.png)"),
            "#image(\"/all-notes/pics/x.png\", alt: \"alt text\");\n"
        );
    }

    #[test]
    fn image_with_a_blank_alt_writes_no_alt() {
        // Typst accepts `alt: " "` and records it as a present but blank
        // description, which is worse than none.
        assert_eq!(body("![ ](x.png)"), "#image(\"/all-notes/x.png\");\n");
    }

    #[test]
    fn image_alt_with_quotes_and_backslashes_is_string_escaped() {
        // An unescaped `\u{41}` would reach the PDF as `A`.
        assert_eq!(
            body("![a \\\\u{41} \"q\"](x.png)"),
            "#image(\"/all-notes/x.png\", alt: \"a \\\\u{41} \\\"q\\\"\");\n"
        );
    }

    #[test]
    fn image_alt_with_markup_is_passed_as_plain_text() {
        assert_eq!(
            body("![*bold* and `code`](x.png)"),
            "#image(\"/all-notes/x.png\", alt: \"bold and code\");\n"
        );
    }

    #[test]
    fn image_alt_keeps_typst_markup_characters_literal() {
        // Inside a string literal `#`, `$` and `@` are text, so the
        // markup escapes of `markup_text` would show up as stray backslashes.
        assert_eq!(
            body("![#strong[x] $y @a](x.png)"),
            "#image(\"/all-notes/x.png\", alt: \"#strong[x] $y @a\");\n"
        );
    }

    #[test]
    fn image_alt_with_a_character_reference_newline_becomes_a_space_in_a_list_item() {
        // A raw newline inside the string would be split by the list item's
        // continuation indent and put spaces into the alt.
        assert_eq!(
            body("- ![a&#10;b](x.png)"),
            "- #image(\"/all-notes/x.png\", alt: \"a b\");\n"
        );
    }

    #[test]
    fn an_image_fragment_is_left_out_of_the_path() {
        // A fragment selects nothing in an embedded image, and typst would
        // look for a file with the `#` in its name.
        assert_eq!(body("![](x.svg#icon)"), "#image(\"/all-notes/x.svg\");\n");
    }

    #[test]
    fn image_with_alt_and_a_fragment_drops_the_fragment_only() {
        assert_eq!(
            body("![icon](x.svg#icon)"),
            "#image(\"/all-notes/x.svg\", alt: \"icon\");\n"
        );
    }

    #[test]
    fn a_link_to_a_local_file_keeps_its_fragment() {
        assert_eq!(
            body("[spec](spec.pdf#page=2)"),
            "#link(\"spec.pdf#page=2\")[spec];\n"
        );
    }

    #[test]
    fn image_with_empty_alt() {
        assert_eq!(body("![](x.png)"), "#image(\"/all-notes/x.png\");\n");
    }

    #[test]
    fn image_alt_with_markup_flattens_to_plain_text() {
        // Only the remote form emits the alt, and it emits the flattened text
        // with the emphasis markers gone.
        let (out, warnings) = emit(
            "![*bold* and `code`](https://h/x.png)",
            &[],
            ASSET_BASE,
            None,
        );
        assert_eq!(out, "#link(\"https://h/x.png\")[bold and code];\n");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_note_relative_image_resolves_under_the_notes_directory() {
        // The compiler's root is the vault, so a bare filename beside the note
        // must be spelled out from the root or it is looked for at the wrong
        // place (ADR 0045).
        assert_eq!(
            resolve_asset("/all-notes", "diagram.png"),
            "/all-notes/diagram.png"
        );
        assert_eq!(
            resolve_asset("/all-notes", "./pics/x.png"),
            "/all-notes/pics/x.png"
        );
    }

    #[test]
    fn an_image_path_climbing_out_of_the_notes_directory_reaches_the_vault() {
        // The same rewrite gives a note access to a shared vault asset, which
        // it could not reach at all while the root was the notes directory.
        assert_eq!(
            resolve_asset("/all-notes", "../assets/logo.svg"),
            "/assets/logo.svg"
        );
    }

    #[test]
    fn an_already_absolute_image_path_is_resolved_from_the_vault_root() {
        // The author is addressing the vault root directly, so the note's
        // directory plays no part.
        assert_eq!(
            resolve_asset("/all-notes", "/assets/logo.svg"),
            "/assets/logo.svg"
        );
    }

    #[test]
    fn a_root_absolute_image_path_is_normalized() {
        assert_eq!(
            resolve_asset("/all-notes", "/assets/./logo.svg"),
            "/assets/logo.svg"
        );
    }

    #[test]
    fn a_root_absolute_image_path_climbing_out_is_left_for_the_compiler() {
        assert_eq!(
            resolve_asset("/all-notes", "/../outside.png"),
            "/../outside.png"
        );
    }

    #[test]
    fn an_image_path_escaping_the_root_is_left_for_the_compiler_to_reject() {
        // It cannot be expressed as a root-absolute path, and inventing one
        // would point at a different file than the author wrote.
        assert_eq!(
            resolve_asset("/all-notes", "../../outside.png"),
            "../../outside.png"
        );
    }

    #[test]
    fn a_remote_image_url_is_never_rewritten_as_a_path() {
        // The remote arm runs before any path resolution, so the URL survives
        // intact into the degraded link.
        let (out, _) = emit("![x](https://host/pic.png)", &[], ASSET_BASE, None);
        assert!(
            out.contains("https://host/pic.png") && !out.contains("/all-notes/https"),
            "a remote URL was treated as a path: {out}"
        );
    }

    #[test]
    fn remote_image_degrades_to_a_link_with_a_warning() {
        let (out, warnings) = emit("![caption](http://host/pic.png)", &[], ASSET_BASE, None);
        assert_eq!(out, "#link(\"http://host/pic.png\")[caption];\n");
        assert_eq!(
            warnings[0].message,
            "remote image cannot be embedded; linked instead: http://host/pic.png"
        );
    }

    #[test]
    fn remote_image_with_empty_alt_labels_with_the_url() {
        let (out, _) = emit("![](https://host/pic.png)", &[], ASSET_BASE, None);
        assert_eq!(
            out,
            "#link(\"https://host/pic.png\")[https\\:\\/\\/host\\/pic\\.png];\n"
        );
    }

    #[test]
    fn image_inside_a_link_does_not_crash() {
        // The image renders its degraded or local form inside the link label.
        assert_eq!(
            body("[![alt](local.png)](https://target)"),
            "#link(\"https://target\")[#image(\"/all-notes/local.png\", alt: \"alt\");];\n"
        );
    }

    // =====================================================================
    // Footnotes
    // =====================================================================

    #[test]
    fn footnote_definition_after_the_reference() {
        insta::assert_snapshot!(body("A claim.[^1]\n\n[^1]: The source."));
    }

    #[test]
    fn footnote_definition_before_the_reference() {
        insta::assert_snapshot!(body("[^1]: The source.\n\nA claim.[^1]"));
    }

    /// Footnote labels are `<ntropy-fn-N>` with N the footnote's position in
    /// the walk's first-reference order; the tests spell them out.
    #[test]
    fn a_footnote_referenced_twice_is_drawn_at_the_first_reference_only() {
        assert_eq!(
            body("x[^n] y[^n]\n\n[^n]: Shared."),
            "x#[#footnote[Shared\\.]<ntropy-fn-1>]; y#footnote(<ntropy-fn-1>);\n"
        );
    }

    #[test]
    fn a_footnote_referencing_itself_refers_back_to_its_own_label() {
        assert_eq!(
            body("A[^a] and again[^a].\n\n[^a]: B[^a]"),
            "A#[#footnote[B#footnote(<ntropy-fn-1>);]<ntropy-fn-1>]; and again#footnote(<ntropy-fn-1>);\\.\n"
        );
    }

    #[test]
    fn two_footnotes_referencing_each_other_end_in_a_label_reference() {
        assert_eq!(
            body("x[^a]\n\n[^a]: A[^b]\n\n[^b]: B[^a]"),
            "x#[#footnote[A#[#footnote[B#footnote(<ntropy-fn-1>);]<ntropy-fn-2>];]<ntropy-fn-1>];\n"
        );
    }

    #[test]
    fn a_chain_of_footnotes_nests_each_one_once() {
        assert_eq!(
            body("x[^a]\n\n[^a]: A[^b]\n\n[^b]: B[^c]\n\n[^c]: C"),
            "x#[#footnote[A#[#footnote[B#[#footnote[C]<ntropy-fn-3>];]<ntropy-fn-2>];]<ntropy-fn-1>];\n"
        );
    }

    #[test]
    fn the_first_reference_in_the_text_draws_the_footnote_whatever_its_number() {
        // `[^b]` is numbered after `[^a]` by the walk, yet its first place in
        // the emitted text is inside the content of `[^a]`.
        assert_eq!(
            body("x[^a] y[^b]\n\n[^a]: A[^b]\n\n[^b]: B"),
            "x#[#footnote[A#[#footnote[B]<ntropy-fn-2>];]<ntropy-fn-1>]; y#footnote(<ntropy-fn-2>);\n"
        );
    }

    #[test]
    fn a_footnote_first_referenced_in_a_heading_keeps_its_label_on_the_footnote() {
        // A bare label after the call would attach to the heading, and a later
        // `#footnote(<label>)` could not find a footnote.
        assert_eq!(
            body("# Head[^a]\n\nlater[^a]\n\n[^a]: A"),
            "= Head#[#footnote[A]<ntropy-fn-1>]; <ntropy-h-1>\n\nlater#footnote(<ntropy-fn-1>);\n"
        );
    }

    #[test]
    fn a_heading_inside_a_footnote_keeps_the_label_of_a_footnote_in_it() {
        assert_eq!(
            body("x[^a] y[^b]\n\n[^a]:\n    # H[^b]\n\n[^b]: B"),
            "x#[#footnote[= H#[#footnote[B]<ntropy-fn-2>]; <ntropy-h-1>]<ntropy-fn-1>]; y#footnote(<ntropy-fn-2>);\n"
        );
    }

    #[test]
    fn a_parenthesis_after_a_footnote_reference_stays_text() {
        // `#footnote(<label>)(x)` would read as a call on the footnote, so the
        // forms end in `;`.
        assert_eq!(
            body("a[^n](see p. 3) b[^n](see p. 4)\n\n[^n]: N"),
            "a#[#footnote[N]<ntropy-fn-1>];(see p\\. 3) b#footnote(<ntropy-fn-1>);(see p\\. 4)\n"
        );
    }

    #[test]
    fn two_references_to_an_undefined_footnote_render_its_literal_twice() {
        assert_eq!(body("a[^g] b[^g]"), "a\\[^g\\] b\\[^g\\]\n");
    }

    #[test]
    fn undefined_footnote_reference_renders_as_its_literal_text() {
        // GFM shows the raw reference when nothing defines it; the literal is
        // escaped as markup.
        // `^` is not markup-active, so only the brackets are escaped.
        assert_eq!(body("A gap.[^missing]"), "A gap\\.\\[^missing\\]\n");
    }

    #[test]
    fn unreferenced_footnote_definition_produces_no_output() {
        // The definition parses but nothing points at it, so the body is empty.
        assert_eq!(body("[^orphan]: Nobody cites me."), "");
    }

    #[test]
    fn footnote_definition_with_block_content() {
        // A definition may carry a list and a code block; both survive inside
        // the `#footnote[...]` content.
        let out =
            body("Ref.[^b]\n\n[^b]: Intro:\n\n    - one\n    - two\n\n    ```\n    code\n    ```");
        insta::assert_snapshot!(out);
        // A code fence nested in a content block is the riskiest form, so the
        // output is parsed to prove it stays valid Typst.
        let root = typst_syntax::parse(&out);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(errors.is_empty(), "parse errors in {out:?}: {errors:?}");
    }

    // =====================================================================
    // Text right after an inline call
    // =====================================================================
    //
    // Typst reads a `(` after `#func[...]` or `#func(...)` as more arguments
    // of the call, and a `[` as one more content argument. Every inline call
    // therefore ends in `;`, which the markup parser consumes. Each case below
    // is a test of its own so that a failure names its construct.

    /// One test per construct and following character: `$input` emits as
    /// `$expected`.
    macro_rules! call_is_closed {
        ($($name:ident: $input:expr => $expected:expr;)*) => {
            $(
                #[test]
                fn $name() {
                    assert_eq!(body($input), $expected);
                }
            )*
        };
    }

    call_is_closed! {
        strong_before_a_parenthesis: "**b**(x)" => "#strong[b];(x)\n";
        strong_before_a_bracket: "**b**[x]" => "#strong[b];\\[x\\]\n";
        emph_before_a_parenthesis: "*e*(x)" => "#emph[e];(x)\n";
        emph_before_a_bracket: "*e*[x]" => "#emph[e];\\[x\\]\n";
        strike_before_a_parenthesis: "~~s~~(x)" => "#strike[s];(x)\n";
        strike_before_a_bracket: "~~s~~[x]" => "#strike[s];\\[x\\]\n";
        link_before_a_parenthesis: "[l](https://example.org)(x)"
            => "#link(\"https://example.org\")[l];(x)\n";
        link_before_a_bracket: "[l](https://example.org)[x]"
            => "#link(\"https://example.org\")[l];\\[x\\]\n";
        autolink_before_a_parenthesis: "<https://example.org>(x)"
            => "#link(\"https://example.org\")[https\\:\\/\\/example\\.org];(x)\n";
        autolink_before_a_bracket: "<https://example.org>[x]"
            => "#link(\"https://example.org\")[https\\:\\/\\/example\\.org];\\[x\\]\n";
        inline_code_before_a_parenthesis: "`c`(x)" => "#raw(\"c\");(x)\n";
        inline_code_before_a_bracket: "`c`[x]" => "#raw(\"c\");\\[x\\]\n";
        hard_break_before_a_parenthesis: "a\\\n(x)" => "a#linebreak();(x)\n";
        hard_break_before_a_bracket: "a\\\n[x]" => "a#linebreak();\\[x\\]\n";
        image_before_a_parenthesis: "![](x.png)(y)" => "#image(\"/all-notes/x.png\");(y)\n";
        image_before_a_bracket: "![](x.png)[y]" => "#image(\"/all-notes/x.png\");\\[y\\]\n";
        remote_image_before_a_parenthesis: "![alt](https://h/x.png)(y)"
            => "#link(\"https://h/x.png\")[alt];(y)\n";
        remote_image_before_a_bracket: "![alt](https://h/x.png)[y]"
            => "#link(\"https://h/x.png\")[alt];\\[y\\]\n";
    }

    #[test]
    fn note_link_before_a_parenthesis() {
        let input = "[l](01ARZ3NDEKTSV4RRFFQ69G5FAV-old.md)(x)";
        assert_eq!(
            emit(input, &resolved_links(input), ASSET_BASE, None).0,
            "#link(\"target-title.pdf\")[#notelink[Target Title]];(x)\n"
        );
    }

    #[test]
    fn note_link_before_a_bracket() {
        let input = "[l](01ARZ3NDEKTSV4RRFFQ69G5FAV-old.md)[x]";
        assert_eq!(
            emit(input, &resolved_links(input), ASSET_BASE, None).0,
            "#link(\"target-title.pdf\")[#notelink[Target Title]];\\[x\\]\n"
        );
    }

    #[test]
    fn a_task_marker_needs_no_terminator_before_a_parenthesis() {
        // The space that follows the marker ends the expression, so `task`
        // is the one inline call without a `;`.
        assert_eq!(body("- [ ] (x)"), "- #task(done: false) (x)\n");
    }

    #[test]
    fn a_checked_task_marker_needs_no_terminator_before_a_parenthesis() {
        assert_eq!(body("- [x] (x)"), "- #task(done: true) (x)\n");
    }

    #[test]
    fn a_semicolon_in_the_text_after_a_call_stays_text() {
        // The call's own `;` is consumed by the parser; the one in the note's
        // text is a second character and stays.
        assert_eq!(body("**b**;x"), "#strong[b];;x\n");
    }

    // =====================================================================
    // Links to a heading of the same note
    // =====================================================================
    //
    // A heading ends in `<ntropy-h-N>`, N the number of the heading in the
    // order the output draws them; a link to its fragment names that label.

    /// The warnings an emit raised, for the cases whose contract is the
    /// warning.
    fn warning_messages(input: &str) -> Vec<String> {
        emit(input, &[], ASSET_BASE, None)
            .1
            .into_iter()
            .map(|warning| warning.message)
            .collect()
    }

    #[test]
    fn a_link_to_a_heading_points_at_the_headings_label() {
        assert_eq!(
            body("## Lists\n\nText.\n\n## Other\n\n[back](#lists)"),
            "== Lists <ntropy-h-1>\n\nText\\.\n\n== Other <ntropy-h-2>\n\n#link(<ntropy-h-1>)[back];\n"
        );
    }

    #[test]
    fn a_link_to_a_heading_further_down_points_forward() {
        assert_eq!(
            body("[down](#later)\n\n## Later"),
            "#link(<ntropy-h-1>)[down];\n\n== Later <ntropy-h-1>\n"
        );
    }

    #[test]
    fn a_link_to_an_unknown_fragment_is_plain_text() {
        assert_eq!(body("[missing](#nope)"), "missing\n");
    }

    #[test]
    fn a_link_to_an_unknown_fragment_raises_a_warning() {
        assert_eq!(
            warning_messages("[missing](#nope)"),
            ["link to `#nope` names no heading of the note; shown as text"]
        );
    }

    #[test]
    fn an_unknown_fragment_with_a_known_heading_elsewhere_still_warns() {
        assert_eq!(
            body("## Section\n\n[a](#section) [b](#section-2)"),
            "== Section <ntropy-h-1>\n\n#link(<ntropy-h-1>)[a]; b\n"
        );
        assert_eq!(
            warning_messages("## Section\n\n[a](#section) [b](#section-2)").len(),
            1
        );
    }

    #[test]
    fn a_link_to_a_heading_inside_a_footnote_points_at_the_headings_label() {
        assert_eq!(
            body("x[^a] see [h](#inner)\n\n[^a]:\n    # Inner\n"),
            "x#[#footnote[= Inner <ntropy-h-1>]<ntropy-fn-1>]; see #link(<ntropy-h-1>)[h];\n"
        );
    }

    #[test]
    fn a_link_to_a_heading_of_an_undrawn_footnote_is_plain_text_with_a_warning() {
        let input = "see [h](#inner)\n\n[^a]:\n    # Inner\n";
        assert_eq!(body(input), "see h\n");
        assert_eq!(warning_messages(input).len(), 1);
    }

    #[test]
    fn repeated_heading_text_gets_suffixed_ids_that_links_can_name() {
        assert_eq!(
            body("## Same\n\n## Same\n\n[a](#same) [b](#same-1)"),
            "== Same <ntropy-h-1>\n\n== Same <ntropy-h-2>\n\n#link(<ntropy-h-1>)[a]; #link(<ntropy-h-2>)[b];\n"
        );
    }

    #[test]
    fn a_heading_with_markup_is_labelled_by_its_plain_text() {
        assert_eq!(
            body("## The *big* `code`\n\n[x](#the-big-code)"),
            "== The #emph[big]; #raw(\"code\"); <ntropy-h-1>\n\n#link(<ntropy-h-1>)[x];\n"
        );
    }

    #[test]
    fn a_link_inside_a_heading_points_at_another_heading() {
        assert_eq!(
            body("## See [target](#target)\n\n## Target"),
            "== See #link(<ntropy-h-2>)[target]; <ntropy-h-1>\n\n== Target <ntropy-h-2>\n"
        );
    }

    #[test]
    fn a_percent_encoded_fragment_names_the_heading_it_decodes_to() {
        assert_eq!(
            body("## Größe\n\n[g](#gr%C3%B6%C3%9Fe)"),
            "== Größe <ntropy-h-1>\n\n#link(<ntropy-h-1>)[g];\n"
        );
    }

    #[test]
    fn a_heading_with_a_character_typst_rejects_in_a_label_still_gets_a_valid_label() {
        // `²` and `½` are alphanumeric, so they stay in the id, yet
        // `<ntropy-h-area-m²>` would fail the whole compile.
        assert_eq!(
            body("## Area m²\n\n## CO₂ and ½\n\n[a](#area-m²) [b](#co₂-and-½)"),
            "== Area m² <ntropy-h-1>\n\n== CO₂ and ½ <ntropy-h-2>\n\n#link(<ntropy-h-1>)[a]; #link(<ntropy-h-2>)[b];\n"
        );
    }

    #[test]
    fn a_fragment_matches_the_heading_id_case_sensitively() {
        let input = "## Lists\n\n[x](#Lists)";
        assert_eq!(body(input), "== Lists <ntropy-h-1>\n\nx\n");
        assert_eq!(warning_messages(input).len(), 1);
    }

    #[test]
    fn a_fragment_with_a_space_names_no_heading() {
        let input = "## My heading\n\n[x](<#my heading>)";
        assert_eq!(body(input), "== My heading <ntropy-h-1>\n\nx\n");
        assert_eq!(warning_messages(input).len(), 1);
    }

    #[test]
    fn a_link_to_the_dropped_title_heading_is_plain_text_with_a_warning() {
        let input = "# Rust Tips\n\n[t](#rust-tips)";
        assert_eq!(emit(input, &[], ASSET_BASE, Some("Rust Tips")).0, "t\n");
        assert_eq!(emit(input, &[], ASSET_BASE, Some("Rust Tips")).1.len(), 1);
    }

    #[test]
    fn a_fragment_that_a_footnote_id_reserves_names_no_heading() {
        // The heading takes `fn-1-1`, as in the HTML output of the note, so
        // `#fn-1` names no heading.
        let input = "# Fn 1\n\ntext[^1]\n\n[^1]: n\n\n[x](#fn-1-1) [y](#fn-1)";
        assert_eq!(
            body(input),
            "= Fn 1 <ntropy-h-1>\n\ntext#[#footnote[n]<ntropy-fn-1>];\n\n#link(<ntropy-h-1>)[x]; y\n"
        );
        assert_eq!(warning_messages(input).len(), 1);
    }

    #[test]
    fn a_link_to_the_top_without_such_a_heading_goes_to_the_first_page() {
        assert_eq!(
            body("[up](#top) [there](#TOP) [start](#)"),
            "#link((page: 1, x: 0pt, y: 0pt))[up]; #link((page: 1, x: 0pt, y: 0pt))[there]; #link((page: 1, x: 0pt, y: 0pt))[start];\n"
        );
        assert!(warning_messages("[up](#top)").is_empty());
    }

    #[test]
    fn a_heading_named_top_wins_over_the_top_of_the_page() {
        assert_eq!(
            body("## Top\n\n[up](#top)"),
            "== Top <ntropy-h-1>\n\n#link(<ntropy-h-1>)[up];\n"
        );
    }

    // The note links to the note itself (ADR 0086 form) with an anchor.

    /// Emit `input` as the note whose own id every link of it names, the
    /// links' target title being the note's own `title`.
    fn emit_self_linking(input: &str, title: &str) -> (String, Vec<Warning>) {
        let mut links = resolved_links(input);
        for link in &mut links {
            if let Some(target) = &mut link.target {
                target.title = title.to_string();
            }
        }
        let own = links.first().map(|link| link.id);
        emit_for_note(input, &links, ASSET_BASE, Some(title), own)
    }

    const SELF_LINK: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV-self-link.md";

    #[test]
    fn a_note_link_to_a_heading_of_its_own_note_points_at_the_headings_label() {
        let input = format!("## Lists\n\n[back to the *lists*]({SELF_LINK}#lists)");
        assert_eq!(
            emit_self_linking(&input, "Self link").0,
            "== Lists <ntropy-h-1>\n\n#link(<ntropy-h-1>)[#notelink[back to the #emph[lists];]];\n"
        );
    }

    #[test]
    fn a_note_link_to_its_own_note_with_no_such_heading_is_styled_text_and_a_warning() {
        let input = format!("## Lists\n\n[back]({SELF_LINK}#nope)");
        let (out, warnings) = emit_self_linking(&input, "Self link");
        assert_eq!(out, "== Lists <ntropy-h-1>\n\n#notelink[back];\n");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_note_link_to_its_own_note_with_blank_text_shows_the_notes_title() {
        let input = format!("## Lists\n\n[]({SELF_LINK}#lists)");
        assert_eq!(
            emit_self_linking(&input, "Self link").0,
            "== Lists <ntropy-h-1>\n\n#link(<ntropy-h-1>)[#notelink[Self link]];\n"
        );
    }

    #[test]
    fn a_note_link_to_the_top_of_its_own_note_goes_to_the_first_page() {
        let input = format!("[up]({SELF_LINK}#top)");
        let (out, warnings) = emit_self_linking(&input, "Self link");
        assert_eq!(out, "#link((page: 1, x: 0pt, y: 0pt))[#notelink[up]];\n");
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_percent_encoded_anchor_of_a_note_link_to_its_own_note_names_the_heading() {
        let input = format!("## Größe\n\n[g]({SELF_LINK}#gr%C3%B6%C3%9Fe)");
        assert_eq!(
            emit_self_linking(&input, "Self link").0,
            "== Größe <ntropy-h-1>\n\n#link(<ntropy-h-1>)[#notelink[g]];\n"
        );
    }

    #[test]
    fn the_anchor_of_a_note_link_to_its_own_note_is_case_sensitive() {
        let input = format!("## Lists\n\n[x]({SELF_LINK}#Lists)");
        let (out, warnings) = emit_self_linking(&input, "Self link");
        assert_eq!(out, "== Lists <ntropy-h-1>\n\n#notelink[x];\n");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_note_link_to_the_skipped_title_heading_of_its_own_note_warns() {
        let input = format!("# Self link\n\n[t]({SELF_LINK}#self-link)");
        let links = resolved_links(&input);
        let own = links.first().map(|link| link.id);
        let (out, warnings) = emit_for_note(&input, &links, ASSET_BASE, Some("Self link"), own);
        assert_eq!(out, "#notelink[t];\n");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_note_link_to_its_own_note_inside_a_heading_and_a_footnote_points_at_a_label() {
        let input = format!(
            "## Lists\n\n## See [back]({SELF_LINK}#lists)\n\ntext[^a]\n\n[^a]: [again]({SELF_LINK}#lists)"
        );
        assert_eq!(
            emit_self_linking(&input, "Self link").0,
            "== Lists <ntropy-h-1>\n\n== See #link(<ntropy-h-1>)[#notelink[back]]; <ntropy-h-2>\n\ntext#[#footnote[#link(<ntropy-h-1>)[#notelink[again]];]<ntropy-fn-1>];\n"
        );
    }

    #[test]
    fn a_note_link_to_its_own_note_without_an_anchor_stays_a_link_to_the_artifact() {
        let input = format!("[me]({SELF_LINK})");
        assert_eq!(
            emit_self_linking(&input, "Self link").0,
            "#link(\"target-title.pdf\")[#notelink[Self link]];\n"
        );
    }

    #[test]
    fn a_note_link_to_its_own_note_with_an_empty_anchor_stays_a_link_to_the_artifact() {
        // An empty anchor names nothing; the artifact opens at its first
        // page, which is the top of the note.
        let input = format!("[me]({SELF_LINK}#)");
        assert_eq!(
            emit_self_linking(&input, "Self link").0,
            "#link(\"target-title.pdf\")[#notelink[Self link]];\n"
        );
    }

    #[test]
    fn a_note_link_with_an_anchor_to_another_note_still_drops_the_anchor() {
        let input = format!("## Lists\n\n[there]({SELF_LINK}#lists)");
        let links = resolved_links(&input);
        let elsewhere = Some(crate::id::Id::from_timestamp_ms(0));
        assert_eq!(
            emit_for_note(&input, &links, ASSET_BASE, None, elsewhere).0,
            "== Lists <ntropy-h-1>\n\n#link(\"target-title.pdf\")[#notelink[there]];\n"
        );
    }

    #[test]
    fn without_an_own_id_an_anchored_note_link_keeps_dropping_the_anchor() {
        let input = format!("## Lists\n\n[back]({SELF_LINK}#lists)");
        assert_eq!(
            body_with_links(&input),
            "== Lists <ntropy-h-1>\n\n#link(\"target-title.pdf\")[#notelink[back]];\n"
        );
    }

    #[test]
    fn a_link_to_a_heading_keeps_the_markup_of_its_label() {
        assert_eq!(
            body("## Head\n\n[see *this*](#head)"),
            "== Head <ntropy-h-1>\n\n#link(<ntropy-h-1>)[see #emph[this];];\n"
        );
    }

    // =====================================================================
    // Autolinks (linkify over text)
    // =====================================================================

    #[test]
    fn bare_url_mid_sentence_is_linked() {
        assert_eq!(
            body("visit https://example.com now"),
            "visit #link(\"https://example.com\")[https\\:\\/\\/example\\.com]; now\n"
        );
    }

    #[test]
    fn www_url_is_linked_with_an_https_target() {
        // GFM prefixes a scheme-less `www.` host with `https://` for the
        // target while the label stays as written.
        assert_eq!(
            body("see www.example.com today"),
            "see #link(\"https://www.example.com\")[www\\.example\\.com]; today\n"
        );
    }

    #[test]
    fn trailing_sentence_punctuation_is_not_swallowed_by_the_url() {
        // linkify keeps the trailing period out of the URL; it renders as
        // ordinary escaped text after the link.
        assert_eq!(
            body("end at https://example.com."),
            "end at #link(\"https://example.com\")[https\\:\\/\\/example\\.com];\\.\n"
        );
    }

    #[test]
    fn a_scheme_less_dotted_word_is_not_autolinked() {
        // `report.txt` is a linkify match only because scheme is optional; the
        // filter rejects it, so it stays plain escaped text.
        assert_eq!(body("open report.txt here"), "open report\\.txt here\n");
    }

    #[test]
    fn a_url_inside_a_link_label_is_not_re_linked() {
        // The label text is not autolinked, so the inner URL stays plain.
        assert_eq!(
            body("[https://example.com](https://target)"),
            "#link(\"https://target\")[https\\:\\/\\/example\\.com];\n"
        );
    }

    #[test]
    fn a_url_inside_inline_code_is_untouched() {
        assert_eq!(
            body("`https://example.com`"),
            "#raw(\"https://example.com\");\n"
        );
    }

    #[test]
    fn an_email_in_text_becomes_a_mailto_link() {
        assert_eq!(
            body("mail me at foo@example.org please"),
            "mail me at #link(\"mailto:foo@example.org\")[foo\\@example\\.org]; please\n",
        );
    }

    // =====================================================================
    // Raw HTML
    // =====================================================================

    #[test]
    fn raw_html_block_is_dropped_with_a_warning() {
        let (out, warnings) = emit(
            "<div class=\"box\">\ncontent\n</div>",
            &[],
            ASSET_BASE,
            None,
        );
        assert_eq!(out, "");
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .message
                .starts_with("dropped raw HTML: <div class=\"box\">"),
            "unexpected message: {:?}",
            warnings[0].message
        );
    }

    #[test]
    fn inline_raw_html_is_dropped_with_a_warning() {
        let (out, warnings) = emit("text <span>x</span> more", &[], ASSET_BASE, None);
        // The surrounding prose survives; only the tags are dropped.
        assert_eq!(out, "text x more\n");
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].message, "dropped raw HTML: <span>");
        assert_eq!(warnings[1].message, "dropped raw HTML: </span>");
    }

    #[test]
    fn a_long_html_fragment_is_truncated_in_the_warning() {
        let long_attr = "x".repeat(200);
        let input = format!("<div data-x=\"{long_attr}\">\ntext\n</div>");
        let warnings = warnings(&input);
        let message = &warnings[0].message;
        // Prefix plus 60 fragment characters plus the ellipsis marker.
        assert!(message.ends_with('…'), "expected truncation: {message:?}");
        let fragment = message.strip_prefix("dropped raw HTML: ").expect("prefix");
        assert_eq!(fragment.chars().count(), 61);
    }

    // =====================================================================
    // Kitchen sink
    // =====================================================================

    #[test]
    fn kitchen_sink_combines_every_inline_construct() {
        let input = "\
# A *heading* with `code`

Prose with **strong**, _emph_, ~~strike~~, an autolink https://example.com,
an email a@b.org, and a [note](noteid) plus an [external](https://ext).

A footnote reference.[^fn] And a dangling [display *text*](danglingid).

![local](pic.png) then ![remote](https://host/img.png).

<div>raw block</div>

- item with `inline`
- item with [link](https://li.st)

[^fn]: Footnote body with *emphasis* and a second line.";
        // `[note](noteid)` spans 116..130; `[display *text*](danglingid)` spans
        // 158..186. Both are resolved through the note-link table.
        let note_span = {
            let start = input.find("[note]").expect("note link present");
            start..start + "[note](noteid)".len()
        };
        let dangling_span = {
            let start = input.find("[display").expect("dangling link present");
            start..start + "[display *text*](danglingid)".len()
        };
        let links = [
            note_link(note_span, Some(("Resolved Title", "resolved-title"))),
            note_link(dangling_span, None),
        ];
        let (out, warnings) = emit(input, &links, ASSET_BASE, None);
        insta::assert_snapshot!(out);
        insta::assert_debug_snapshot!(warnings);
    }

    #[test]
    fn kitchen_sink_output_parses_without_errors() {
        // The same document must be syntactically valid Typst: a parse with no
        // errors proves the emitter never produced malformed markup.
        let input = "\
# A *heading* with `code`

Prose with **strong**, _emph_, ~~strike~~, an autolink https://example.com,
an email a@b.org, and a [note](noteid) plus an [external](https://ext).

A footnote reference.[^fn] And a dangling [display *text*](danglingid).

![local](pic.png) then ![remote](https://host/img.png).

<div>raw block</div>

- item with `inline`
- item with [link](https://li.st)

[^fn]: Footnote body with *emphasis* and a second line.";
        let note_span = {
            let start = input.find("[note]").expect("note link present");
            start..start + "[note](noteid)".len()
        };
        let dangling_span = {
            let start = input.find("[display").expect("dangling link present");
            start..start + "[display *text*](danglingid)".len()
        };
        let links = [
            note_link(note_span, Some(("Resolved Title", "resolved-title"))),
            note_link(dangling_span, None),
        ];
        let (out, _) = emit(input, &links, ASSET_BASE, None);

        let root = typst_syntax::parse(&out);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(errors.is_empty(), "parse errors in {out:?}: {errors:?}");
    }

    // =====================================================================
    // Round-trip verification against the real Typst parser
    // =====================================================================
    //
    // The escaping round-trip is proven exhaustively in the writer's own
    // tests. Here the property is narrower: emitted block markup must parse
    // without errors, and prose must stay prose — no paragraph, list, or
    // other construct may creep in from the emitter's own newlines.

    /// Reassemble plain text from a parse tree and collect any node kind that
    /// is not plain text or a paragraph/line separator. `Markup` containers
    /// are transparent; `Escape` leaves decode to the escaped character.
    fn collect(node: &SyntaxNode, text: &mut String, foreign: &mut Vec<SyntaxKind>) {
        let is_leaf = node.children().next().is_none();
        if is_leaf {
            match node.kind() {
                SyntaxKind::Text | SyntaxKind::Space | SyntaxKind::Parbreak => {
                    text.push_str(node.leaf_text());
                }
                SyntaxKind::Escape => text.extend(node.leaf_text().chars().skip(1)),
                other => foreign.push(other),
            }
        } else {
            if node.kind() != SyntaxKind::Markup {
                foreign.push(node.kind());
            }
            for child in node.children() {
                collect(child, text, foreign);
            }
        }
    }

    #[test]
    fn single_paragraph_prose_reassembles_to_its_input() {
        // Output is pure text (plus a trailing newline), so the full
        // reassembly property applies: Typst reads it back verbatim.
        let input = "Prose with a period. And an em---dash, ellipsis... and 6. shape.";
        let emitted = body(input);

        let root = typst_syntax::parse(emitted.trim_end_matches('\n'));
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(errors.is_empty(), "parse errors in {emitted:?}: {errors:?}");

        let mut text = String::new();
        let mut foreign = Vec::new();
        collect(&root, &mut text, &mut foreign);
        assert!(
            foreign.is_empty(),
            "unexpected constructs {foreign:?} in {emitted:?}"
        );
        assert_eq!(text, input);
    }

    #[test]
    fn multi_paragraph_prose_parses_without_constructs() {
        // With structural newlines the output is no longer pure text, so only
        // the weaker property holds: it parses cleanly and introduces no
        // markup construct beyond paragraph breaks.
        let emitted = body("First paragraph of prose.\n\nSecond paragraph of prose.");

        let root = typst_syntax::parse(&emitted);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(errors.is_empty(), "parse errors in {emitted:?}: {errors:?}");

        let mut text = String::new();
        let mut foreign = Vec::new();
        collect(&root, &mut text, &mut foreign);
        assert!(
            foreign.is_empty(),
            "unexpected constructs {foreign:?} in {emitted:?}"
        );
    }

    // =====================================================================
    // The title heading
    // =====================================================================

    #[test]
    fn a_leading_h1_equal_to_the_title_is_dropped() {
        // The block after the dropped heading starts the body: no separator
        // is spent on the heading, and a later repeat of the title is kept.
        assert_eq!(
            body_titled("# Rust Tips\n\nBody.\n\n# Rust Tips\n", "Rust Tips"),
            "Body\\.\n\n= Rust Tips <ntropy-h-1>\n"
        );
    }

    #[test]
    fn a_title_heading_stays_when_it_differs_or_is_not_first() {
        let kept = |input: &str| body_titled(input, "Rust Tips");
        assert_eq!(kept("# Rust tips\n"), "= Rust tips <ntropy-h-1>\n");
        assert_eq!(kept("## Rust Tips\n"), "== Rust Tips <ntropy-h-1>\n");
        assert_eq!(
            kept("Intro.\n\n# Rust Tips\n"),
            "Intro\\.\n\n= Rust Tips <ntropy-h-1>\n"
        );
    }

    #[test]
    fn a_title_heading_stays_in_a_note_without_a_title() {
        // `ntropy render` always has a title, since frontmatter requires one;
        // this guards the emitter's own contract.
        assert_eq!(body("# Rust Tips\n"), "= Rust Tips <ntropy-h-1>\n");
    }

    #[test]
    fn the_title_matches_the_heading_text_without_surrounding_spaces() {
        assert_eq!(
            body_titled("# Rust Tips\n\nBody.\n", "  Rust Tips "),
            "Body\\.\n"
        );
    }

    #[test]
    fn the_title_matches_the_heading_text_not_its_markup() {
        assert_eq!(
            body_titled("# Rust *Tips*\n\nBody.\n", "Rust Tips"),
            "Body\\.\n"
        );
        // A title is plain text, so one that holds markup never matches.
        assert_eq!(
            body_titled("# Rust *Tips*\n", "Rust *Tips*"),
            "= Rust #emph[Tips]; <ntropy-h-1>\n"
        );
    }

    #[test]
    fn a_heading_that_is_an_image_with_the_title_as_alt_is_dropped() {
        assert_eq!(
            body_titled("# ![Rust Tips](logo.png)\n\nBody.\n", "Rust Tips"),
            "Body\\.\n"
        );
    }

    #[test]
    fn a_body_of_only_the_title_heading_is_empty() {
        assert_eq!(body_titled("# Rust Tips\n", "Rust Tips"), "");
    }

    #[test]
    fn raw_html_before_the_title_heading_does_not_keep_it() {
        // The dropped HTML is not on the page, so the heading opens it.
        let (out, warnings) = emit(
            "<!-- draft -->\n\n# Rust Tips\n\nBody.\n",
            &[],
            ASSET_BASE,
            Some("Rust Tips"),
        );
        assert_eq!(out, "Body\\.\n");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_reference_in_the_dropped_title_heading_draws_no_footnote() {
        assert_eq!(body_titled("# T[^a]\n\nx\n\n[^a]: A", "T"), "x\n");
    }

    #[test]
    fn a_footnote_referenced_in_the_dropped_title_and_the_body_is_drawn_once() {
        assert_eq!(
            body_titled("# T[^a]\n\nx[^a]\n\n[^a]: A", "T"),
            "x#[#footnote[A]<ntropy-fn-1>];\n"
        );
    }
}
