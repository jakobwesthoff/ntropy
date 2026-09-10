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
//! `=` markers, fenced and indented code blocks as content-sized raw fences,
//! bullet and ordered lists as `- `/`n. ` markers with continuation lines
//! indented under the marker, block quotes as `#quote`, GFM callouts as
//! `#callout(kind:)`, tables as `#table`, thematic breaks as `#line`.
//!
//! Inline constructs, all emitted as Typst function calls so that no
//! user-derived character is ever load-bearing markup:
//!
//! - Emphasis, strong, and strikethrough wrap their content in `#emph[…]`,
//!   `#strong[…]`, and `#strike[…]`; the wrappers nest.
//! - Inline code becomes `#raw("…")` with string-literal escaping.
//! - Soft breaks map to a space and hard breaks to `#linebreak()`.
//! - A resolved note link renders as `#link("<target-slug>.pdf")[#notelink[Title]]`,
//!   so the artifact carries a clickable target next to itself (ADR 0044).
//!   Every other link, autolinks included, renders as `#link("target")[label]`.
//! - A local image becomes `#image("path")`, rewritten to a root-absolute
//!   path under the note's directory (ADR 0045); a remote `http(s)` image
//!   cannot be embedded by the offline compiler, so it degrades to
//!   `#link("url")[alt-or-url]` and raises a [`Warning`].
//! - A footnote is inlined at its reference site as `#footnote[…]`.
//! - Raw HTML, blocks and inline fragments alike, is dropped and raises a
//!   [`Warning`] naming the dropped fragment.
//!
//! Every character of user text passes through one of the [`TypstWriter`]'s
//! escaped channels; already-rendered children are spliced back through the
//! unescaped `syntax` channel.

use super::writer::{self, TypstWriter};
use crate::id::Id;
use crate::render::ResolvedLink;
use crate::render::markdown::{
    self, Alignment, BlockQuoteKind, Heading, InlineStyle, List, Output, Table, Warning,
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
pub fn emit(body: &str, links: &[ResolvedLink], asset_base: &str) -> (String, Vec<Warning>) {
    let mut output = TypstOutput { asset_base };
    markdown::walk(body, links, &mut output)
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
        writer.syntax("\")");
        writer.finish()
    }

    fn soft_break(&mut self) -> String {
        " ".to_string()
    }

    fn hard_break(&mut self) -> String {
        "#linebreak()".to_string()
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
        format!("#{call}[{inner}]")
    }

    fn link(&mut self, dest: &str, inner: &str) -> String {
        wrap_link(dest, inner)
    }

    /// `#link("<slug>.pdf")[#notelink[Title]]`, with `slug` string-literal
    /// escaped and `title` escaped as markup text.
    ///
    /// The target is the artifact a default `ntropy render` of the target note
    /// produces in the same directory, so a set of notes rendered together
    /// cross-navigates (ADR 0044). The extension is always `pdf`, never the
    /// extension of the format being produced: the `typst` artifact is the
    /// source of a PDF, and both formats emit identical bytes by design.
    /// `notelink` is defined by the prelude, so a theme can style note links
    /// distinctly from ordinary emphasis.
    fn note_link(&mut self, _id: Id, title: &str, slug: &str) -> String {
        let mut writer = TypstWriter::new();
        writer.syntax("#link(\"");
        writer.string_literal(&format!("{slug}.{NOTE_LINK_EXTENSION}"));
        writer.syntax("\")[#notelink[");
        writer.markup_text(title);
        writer.syntax("]]");
        writer.finish()
    }

    /// A local path becomes `#image("path")`; a remote one cannot be embedded
    /// offline and degrades to a link plus a warning.
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
            writer.syntax("]");
            warnings.push(Warning::new(format!(
                "remote image cannot be embedded; linked instead: {dest}"
            )));
        } else {
            writer.syntax("#image(\"");
            writer.string_literal(&resolve_asset(self.asset_base, dest));
            writer.syntax("\")");
        }
        writer.finish()
    }

    fn footnote(&mut self, content: &str) -> String {
        format!("#footnote[{}]", content.trim_end_matches('\n'))
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

    /// The id is not carried into the document: nothing in the Typst output
    /// links to a heading.
    fn heading(&mut self, heading: &Heading, body: &str) -> String {
        let marker = "=".repeat(heading.level as usize);
        format!("{marker} {body}\n")
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
/// directly and passes through. So does one that climbs out of the root
/// altogether, which cannot be expressed as a root-absolute path; the compiler
/// rejects it by name rather than by an invented substitute.
fn resolve_asset(base: &str, dest: &str) -> String {
    if dest.starts_with('/') {
        return dest.to_string();
    }

    // `base` is root-absolute (`/all-notes`); walking the joined components
    // resolves `.` and `..` textually, which is what Typst's own root-relative
    // lookup does. There is no filesystem here to consult, by design: the
    // output is pure.
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in dest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return dest.to_string();
                }
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

/// A `#link("dest")[inner]` call: `dest` string-literal escaped, `inner`
/// already-escaped markup.
fn wrap_link(dest: &str, inner: &str) -> String {
    let mut writer = TypstWriter::new();
    writer.syntax("#link(\"");
    writer.string_literal(dest);
    writer.syntax("\")[");
    writer.syntax(inner);
    writer.syntax("]");
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
        emit(input, &[], ASSET_BASE).0
    }

    /// Emit and keep only the warnings, for the raw-HTML and remote-image
    /// cases whose contract is the warning rather than the body.
    fn warnings(input: &str) -> Vec<Warning> {
        emit(input, &[], ASSET_BASE).1
    }

    /// Build a note-link table entry from an optional `(title, slug)` target.
    /// The range must equal the Link event's byte span in `input` for the
    /// emitter to treat it as a note link.
    fn note_link(range: Range<usize>, target: Option<(&str, &str)>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: crate::id::Id::from_timestamp_ms(0),
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
        assert_eq!(body("one  \ntwo"), "one#linebreak()two\n");
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
        assert_eq!(body("*a* **b** ~~c~~"), "#emph[a] #strong[b] #strike[c]\n");
    }

    #[test]
    fn underscore_emphasis_matches_asterisk_emphasis() {
        assert_eq!(body("_a_ __b__"), "#emph[a] #strong[b]\n");
    }

    #[test]
    fn nested_span_styling_nests_the_calls() {
        // Strong wrapping emphasis wrapping strikethrough proves the frames
        // stack without the delimiters ever reaching the output.
        assert_eq!(
            body("**bold _italic ~~struck~~_**"),
            "#strong[bold #emph[italic #strike[struck]]]\n"
        );
    }

    #[test]
    fn adjacent_styled_words_stay_separate_calls() {
        // Strong immediately followed by strikethrough, no separator: two
        // independent calls with nothing between them.
        assert_eq!(body("**a**~~b~~"), "#strong[a]#strike[b]\n");
    }

    #[test]
    fn styled_text_still_escapes_markup_active_characters() {
        // The inner text passes through the markup channel, so a `#` inside an
        // emphasis is escaped just as it is in plain prose.
        assert_eq!(body("*a#b*"), "#emph[a\\#b]\n");
    }

    // =====================================================================
    // Inline code
    // =====================================================================

    #[test]
    fn inline_code_becomes_raw_with_string_escaping() {
        assert_eq!(body("`plain`"), "#raw(\"plain\")\n");
    }

    #[test]
    fn inline_code_escapes_quotes_and_backslashes() {
        // The two string-literal escapes apply; the markup set does not.
        assert_eq!(body(r#"`a "b" \c`"#), "#raw(\"a \\\"b\\\" \\\\c\")\n");
    }

    #[test]
    fn inline_code_with_backticks_inside() {
        // A double-backtick span carries a literal backtick, which needs no
        // escaping inside a Typst string.
        assert_eq!(body("`` a`b ``"), "#raw(\"a`b\")\n");
    }

    // =====================================================================
    // Ordinary links
    // =====================================================================

    #[test]
    fn external_link() {
        assert_eq!(
            body("[label](https://example.com)"),
            "#link(\"https://example.com\")[label]\n"
        );
    }

    #[test]
    fn relative_and_anchor_and_mailto_links_pass_the_target_verbatim() {
        assert_eq!(body("[a](./notes/b.md)"), "#link(\"./notes/b.md\")[a]\n");
        assert_eq!(body("[a](#section)"), "#link(\"#section\")[a]\n");
        assert_eq!(
            body("[a](mailto:x@example.org)"),
            "#link(\"mailto:x@example.org\")[a]\n"
        );
    }

    #[test]
    fn link_destination_with_a_quote_is_string_escaped() {
        assert_eq!(
            body(r#"[a](https://example.com/?q="x")"#),
            "#link(\"https://example.com/?q=\\\"x\\\"\")[a]\n"
        );
    }

    #[test]
    fn angle_bracket_url_autolink_is_an_ordinary_link() {
        // The label is the URL passed through the markup channel, so its
        // punctuation is escaped.
        assert_eq!(
            body("<https://example.com>"),
            "#link(\"https://example.com\")[https\\:\\/\\/example\\.com]\n"
        );
    }

    #[test]
    fn angle_bracket_email_autolink_gains_the_mailto_scheme() {
        // pulldown-cmark hands an email autolink through with the bare address
        // as the destination; a PDF link needs the `mailto:` scheme to act on
        // it, so the emitter adds it while the label stays as written.
        assert_eq!(
            body("<foo@example.org>"),
            "#link(\"mailto:foo@example.org\")[foo\\@example\\.org]\n"
        );
    }

    #[test]
    fn link_label_carrying_markup_keeps_the_markup() {
        assert_eq!(body("[*a* b](u)"), "#link(\"u\")[#emph[a] b]\n");
    }

    #[test]
    fn link_inside_emphasis_and_emphasis_inside_link() {
        assert_eq!(body("*[a](u)*"), "#emph[#link(\"u\")[a]]\n");
        assert_eq!(body("[**a**](u)"), "#link(\"u\")[#strong[a]]\n");
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
            emit(input, &links, ASSET_BASE).0,
            "See #link(\"target-title.pdf\")[#notelink[Target Title]] end\\.\n"
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
            emit(input, &links, ASSET_BASE).0,
            "#link(\"old-slug.pdf\")[#notelink[A Renamed Note]]\n"
        );
    }

    #[test]
    fn resolved_note_link_discards_inner_markup_label() {
        // The display text carries markup, but a resolved note link shows the
        // target's title instead, so the inner `#strong` never appears.
        let input = "[**bold** display](id)";
        let links = [note_link(0..22, Some(("The Title", "the-title")))];
        assert_eq!(
            emit(input, &links, ASSET_BASE).0,
            "#link(\"the-title.pdf\")[#notelink[The Title]]\n"
        );
    }

    #[test]
    fn resolved_note_link_title_escapes_markup_active_characters() {
        let input = "[x](id)";
        let links = [note_link(0..7, Some(("a*b [c] #d", "a-b-c-d")))];
        assert_eq!(
            emit(input, &links, ASSET_BASE).0,
            "#link(\"a-b-c-d.pdf\")[#notelink[a\\*b \\[c\\] \\#d]]\n"
        );
    }

    #[test]
    fn unresolved_note_link_re_emits_its_markup_label() {
        // A dangling note link drops the wrapper and keeps the display markup.
        let input = "[*display* text](id)";
        let links = [note_link(0..20, None)];
        assert_eq!(emit(input, &links, ASSET_BASE).0, "#emph[display] text\n");
    }

    #[test]
    fn resolved_link_whose_range_matches_no_event_is_ignored() {
        // The note text already renders literally; a stray table entry that
        // matches no Link event changes nothing.
        let input = "just prose";
        let links = [note_link(0..4, Some(("Title", "title")))];
        assert_eq!(emit(input, &links, ASSET_BASE).0, "just prose\n");
    }

    #[test]
    fn link_event_without_a_matching_entry_is_an_ordinary_link() {
        // Same input as the resolved case, but an empty table: the link falls
        // back to an ordinary `#link`, not a note link.
        let input = "See [my note](abc123) end.";
        assert_eq!(body(input), "See #link(\"abc123\")[my note] end\\.\n");
    }

    // =====================================================================
    // Images
    // =====================================================================

    #[test]
    fn local_image_becomes_an_image_call_without_alt() {
        assert_eq!(
            body("![alt text](pics/x.png)"),
            "#image(\"/all-notes/pics/x.png\")\n"
        );
    }

    #[test]
    fn image_with_empty_alt() {
        assert_eq!(body("![](x.png)"), "#image(\"/all-notes/x.png\")\n");
    }

    #[test]
    fn image_alt_with_markup_flattens_to_plain_text() {
        // Only the remote form emits the alt, and it emits the flattened text
        // with the emphasis markers gone.
        let (out, warnings) = emit("![*bold* and `code`](https://h/x.png)", &[], ASSET_BASE);
        assert_eq!(out, "#link(\"https://h/x.png\")[bold and code]\n");
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
    fn an_already_absolute_image_path_is_left_alone() {
        // The author is addressing the vault root directly.
        assert_eq!(
            resolve_asset("/all-notes", "/assets/logo.svg"),
            "/assets/logo.svg"
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
        let (out, _) = emit("![x](https://host/pic.png)", &[], ASSET_BASE);
        assert!(
            out.contains("https://host/pic.png") && !out.contains("/all-notes/https"),
            "a remote URL was treated as a path: {out}"
        );
    }

    #[test]
    fn remote_image_degrades_to_a_link_with_a_warning() {
        let (out, warnings) = emit("![caption](http://host/pic.png)", &[], ASSET_BASE);
        assert_eq!(out, "#link(\"http://host/pic.png\")[caption]\n");
        assert_eq!(
            warnings[0].message,
            "remote image cannot be embedded; linked instead: http://host/pic.png"
        );
    }

    #[test]
    fn remote_image_with_empty_alt_labels_with_the_url() {
        let (out, _) = emit("![](https://host/pic.png)", &[], ASSET_BASE);
        assert_eq!(
            out,
            "#link(\"https://host/pic.png\")[https\\:\\/\\/host\\/pic\\.png]\n"
        );
    }

    #[test]
    fn image_inside_a_link_does_not_crash() {
        // The image renders its degraded or local form inside the link label.
        assert_eq!(
            body("[![alt](local.png)](https://target)"),
            "#link(\"https://target\")[#image(\"/all-notes/local.png\")]\n"
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

    #[test]
    fn footnote_referenced_twice_inlines_the_content_each_time() {
        insta::assert_snapshot!(body("First.[^n] Second.[^n]\n\n[^n]: Shared."));
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
    // Autolinks (linkify over text)
    // =====================================================================

    #[test]
    fn bare_url_mid_sentence_is_linked() {
        assert_eq!(
            body("visit https://example.com now"),
            "visit #link(\"https://example.com\")[https\\:\\/\\/example\\.com] now\n"
        );
    }

    #[test]
    fn www_url_is_linked_with_an_https_target() {
        // GFM prefixes a scheme-less `www.` host with `https://` for the
        // target while the label stays as written.
        assert_eq!(
            body("see www.example.com today"),
            "see #link(\"https://www.example.com\")[www\\.example\\.com] today\n"
        );
    }

    #[test]
    fn trailing_sentence_punctuation_is_not_swallowed_by_the_url() {
        // linkify keeps the trailing period out of the URL; it renders as
        // ordinary escaped text after the link.
        assert_eq!(
            body("end at https://example.com."),
            "end at #link(\"https://example.com\")[https\\:\\/\\/example\\.com]\\.\n"
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
            "#link(\"https://target\")[https\\:\\/\\/example\\.com]\n"
        );
    }

    #[test]
    fn a_url_inside_inline_code_is_untouched() {
        assert_eq!(
            body("`https://example.com`"),
            "#raw(\"https://example.com\")\n"
        );
    }

    #[test]
    fn an_email_in_text_becomes_a_mailto_link() {
        assert_eq!(
            body("mail me at foo@example.org please"),
            "mail me at #link(\"mailto:foo@example.org\")[foo\\@example\\.org] please\n",
        );
    }

    // =====================================================================
    // Raw HTML
    // =====================================================================

    #[test]
    fn raw_html_block_is_dropped_with_a_warning() {
        let (out, warnings) = emit("<div class=\"box\">\ncontent\n</div>", &[], ASSET_BASE);
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
        let (out, warnings) = emit("text <span>x</span> more", &[], ASSET_BASE);
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
        let (out, warnings) = emit(input, &links, ASSET_BASE);
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
        let (out, _) = emit(input, &links, ASSET_BASE);

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
}
