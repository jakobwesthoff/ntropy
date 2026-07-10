// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The Markdown-to-Typst emitter's block layer: a `pulldown-cmark` event
//! loop that turns a note body into Typst block markup
//! (`docs/design/typst-engine.md`, "Element mapping" and "Writer and
//! context model").
//!
//! # What this layer covers
//!
//! Block constructs: paragraphs, headings, fenced and indented code blocks,
//! bullet and ordered lists (including task lists, nesting, and loose-list
//! continuation blocks), block quotes, GFM callouts, tables, and thematic
//! breaks. Line breaks map to a space (soft) and `#linebreak()` (hard).
//!
//! Inline span styling (emphasis, strong, strikethrough, inline code),
//! links, images, footnotes, and autolink detection are the domain of a
//! separate inline layer. Until that layer exists this emitter is
//! deliberately transparent to those constructs: an inline wrapper's inner
//! text still reaches the output as escaped markup text, so a table cell or
//! heading carrying `**bold**` renders the word `bold` rather than crashing.
//! Footnote definitions and raw HTML blocks are dropped whole; footnote
//! references and raw inline HTML are dropped in place. Math is off, so `$`
//! is ordinary escaped text.
//!
//! # Structure
//!
//! The loop carries a stack of [`Frame`]s, one per open block container. A
//! frame is the natural home for the two facts `pulldown-cmark` 0.13 only
//! exposes at `Start` (fence language, list ordinality, table alignments)
//! and for content a container can only shape once complete (a code fence
//! sized from its content, a table laid out from its rows). Inline text and
//! finished child blocks accumulate in the frame's [`TypstWriter`]; every
//! character of user text passes through one of the writer's escaped
//! channels, and finished child blocks are spliced back through the
//! unescaped `syntax` channel. When a container closes, its frame produces
//! one block string that the parent frame incorporates.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd,
};

use super::writer::{self, TypstWriter};

// =========================================================
// Public entry
// =========================================================

/// Convert a note body to Typst block markup.
///
/// The returned string is the converted body alone; document assembly
/// (prelude, template application) and warning reporting live in the engine
/// layer.
pub fn emit(body: &str) -> String {
    // GitHub's rendered surface: tables, strikethrough, task lists, and
    // footnotes, with `ENABLE_GFM` supplying the callout kinds on block
    // quotes. Math stays off by omission, so `$` never gains meaning.
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;

    let mut emitter = Emitter::new();
    // The offset iterator carries each event's byte span. The block layer has
    // no use for spans yet; iterating it now keeps the seam the inline layer
    // needs (note-link matching) without a later parser-construction change.
    for (event, _span) in Parser::new_ext(body, options).into_offset_iter() {
        emitter.handle(event);
    }
    emitter.finish()
}

// =========================================================
// The structural stack
// =========================================================

/// One open block container. The `body` writer accumulates the container's
/// inline text and its already-emitted child blocks; `nonempty` records
/// whether anything has been written yet, which decides block separation.
struct Frame {
    kind: FrameKind,
    body: TypstWriter,
    nonempty: bool,
}

/// The per-container state the loop must retain between a `Start` and its
/// matching `End`.
enum FrameKind {
    /// The whole document. Its accumulated body is the emitter's output.
    Document,
    Paragraph,
    Heading,
    /// Block quotes and GFM callouts share this frame; the kind arrives again
    /// on the `End` event, so only the accumulated child blocks live here.
    Quote,
    /// A fenced or indented code block. Content is buffered verbatim so the
    /// closing fence can be sized to exceed the longest backtick run inside.
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    List(ListState),
    Item,
    Table(TableState),
    TableCell,
    /// A subtree whose output is thrown away: footnote definitions (deferred
    /// to the inline layer) and raw HTML blocks (dropped by design).
    Discard,
}

/// A list under construction. Items are collected as finished strings and
/// joined at the end so tight and loose lists can differ only in the joiner.
struct ListState {
    /// `true` for ordered lists; the marker then carries an explicit number.
    ordered: bool,
    /// The next ordinal to emit. `pulldown-cmark` reports the real start
    /// number, so `6.` `7.` `8.` survive without a Typst `start:` argument.
    next: u64,
    /// A blank line between two items, or any item wrapped in a paragraph,
    /// makes the whole list loose. Detected when a paragraph opens directly
    /// inside an item.
    loose: bool,
    items: Vec<String>,
}

/// A table under construction. The header fixes the column count and the
/// per-column alignment; body rows shorter than the header are padded.
struct TableState {
    alignments: Vec<Alignment>,
    header: Vec<String>,
    rows: Vec<Vec<String>>,
    /// The row currently receiving cells (header or body alike).
    current: Vec<String>,
}

// =========================================================
// The event loop
// =========================================================

struct Emitter {
    stack: Vec<Frame>,
}

impl Emitter {
    fn new() -> Self {
        Emitter {
            stack: vec![Frame {
                kind: FrameKind::Document,
                body: TypstWriter::new(),
                nonempty: false,
            }],
        }
    }

    fn finish(mut self) -> String {
        let document = self
            .stack
            .pop()
            .expect("the document frame is pushed at construction and never popped");
        document.body.finish()
    }

    /// The innermost open container, the sink for the current event.
    fn top(&mut self) -> &mut Frame {
        self.stack
            .last_mut()
            .expect("the document frame keeps the stack non-empty")
    }

    fn push(&mut self, kind: FrameKind) {
        self.stack.push(Frame {
            kind,
            body: TypstWriter::new(),
            nonempty: false,
        });
    }

    /// Splice a finished child block into the current container. Blocks carry
    /// their own trailing newline; a single separator newline between two
    /// siblings therefore yields the blank line that separates every
    /// block-level construct.
    fn append_block(&mut self, block: &str) {
        let frame = self.top();
        if frame.nonempty {
            frame.body.syntax("\n");
        }
        frame.body.syntax(block);
        frame.nonempty = true;
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            // Inline code is an inline construct; its content reaches the
            // output as escaped text until the inline layer emits `#raw`.
            Event::Code(code) => self.text(&code),
            // Math is disabled, so these never arrive; routing their content
            // as text keeps the match total without a panic path.
            Event::InlineMath(m) | Event::DisplayMath(m) => self.text(&m),
            Event::SoftBreak => self.inline_syntax(" "),
            Event::HardBreak => self.inline_syntax("#linebreak()"),
            Event::Rule => self.append_block("#line(length: 100%)\n"),
            Event::TaskListMarker(checked) => {
                self.inline_syntax(if checked { "☑ " } else { "☐ " });
            }
            // Footnote references and raw HTML are dropped here; the inline
            // layer will re-home footnotes and warn on dropped HTML.
            Event::FootnoteReference(_) | Event::Html(_) | Event::InlineHtml(_) => {}
        }
    }

    // -----------------------------------------------------
    // Text and inline routing
    // -----------------------------------------------------

    /// Route user text to the correct channel for the current container: raw
    /// inside a code block, discarded inside a dropped subtree, escaped
    /// markup everywhere else.
    fn text(&mut self, text: &str) {
        let frame = self.top();
        match &mut frame.kind {
            FrameKind::CodeBlock { content, .. } => content.push_str(text),
            FrameKind::Discard => {}
            _ => {
                frame.body.markup_text(text);
                frame.nonempty = true;
            }
        }
    }

    /// Emitter-owned inline markup (a space for a soft break, the task-list
    /// box, `#linebreak()`), routed like text so it never lands in a code
    /// block's verbatim content or a dropped subtree.
    fn inline_syntax(&mut self, s: &str) {
        let frame = self.top();
        match &frame.kind {
            FrameKind::CodeBlock { .. } | FrameKind::Discard => {}
            _ => {
                frame.body.syntax(s);
                frame.nonempty = true;
            }
        }
    }

    // -----------------------------------------------------
    // Start events
    // -----------------------------------------------------

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                // A paragraph opening directly inside an item is the signal
                // that its list is loose (CommonMark wraps loose-item content
                // in paragraphs, tight-item content bare). The enclosing list
                // is always the frame directly beneath that item.
                let depth = self.stack.len();
                if depth >= 2
                    && matches!(self.stack[depth - 1].kind, FrameKind::Item)
                    && let FrameKind::List(list) = &mut self.stack[depth - 2].kind
                {
                    list.loose = true;
                }
                self.push(FrameKind::Paragraph);
            }
            Tag::Heading { .. } => self.push(FrameKind::Heading),
            Tag::BlockQuote(_) => self.push(FrameKind::Quote),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => language_tag(&info),
                    CodeBlockKind::Indented => None,
                };
                self.push(FrameKind::CodeBlock {
                    language,
                    content: String::new(),
                });
            }
            Tag::List(start) => self.push(FrameKind::List(ListState {
                ordered: start.is_some(),
                next: start.unwrap_or(0),
                loose: false,
                items: Vec::new(),
            })),
            Tag::Item => self.push(FrameKind::Item),
            Tag::Table(alignments) => self.push(FrameKind::Table(TableState {
                alignments,
                header: Vec::new(),
                rows: Vec::new(),
                current: Vec::new(),
            })),
            // Head and body rows both fill `current`; it is drained when the
            // row ends, so clearing it here is a defensive reset.
            Tag::TableHead | Tag::TableRow => {
                if let FrameKind::Table(table) = &mut self.top().kind {
                    table.current.clear();
                }
            }
            Tag::TableCell => self.push(FrameKind::TableCell),
            // Dropped whole: their inner block content accumulates in a
            // throwaway frame and is discarded when the tag ends.
            Tag::FootnoteDefinition(_) | Tag::HtmlBlock => self.push(FrameKind::Discard),
            // Inline span styling and links are transparent for now: no
            // frame, so their inner text flows on as escaped markup.
            Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::Link { .. }
            | Tag::Image { .. } => {}
            // Not enabled in the parser options, so never produced.
            Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    // -----------------------------------------------------
    // End events
    // -----------------------------------------------------

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                let body = self.pop().body.finish();
                self.append_block(&format!("{body}\n"));
            }
            TagEnd::Heading(level) => {
                let body = self.pop().body.finish();
                let marker = "=".repeat(level as usize);
                self.append_block(&format!("{marker} {body}\n"));
            }
            TagEnd::BlockQuote(kind) => {
                let children = self.pop().body.finish();
                self.append_block(&wrap_quote(kind, &children));
            }
            TagEnd::CodeBlock => {
                let frame = self.pop();
                let FrameKind::CodeBlock { language, content } = frame.kind else {
                    unreachable!("a code-block end closes a code-block frame");
                };
                self.append_block(&render_code_block(language.as_deref(), &content));
            }
            TagEnd::List(_) => {
                let frame = self.pop();
                let FrameKind::List(list) = frame.kind else {
                    unreachable!("a list end closes a list frame");
                };
                let joiner = if list.loose { "\n\n" } else { "\n" };
                self.append_block(&format!("{}\n", list.items.join(joiner)));
            }
            TagEnd::Item => {
                let body = self.pop().body.finish();
                let body = body.trim_end_matches('\n');
                // The marker, and thus the continuation indent, is a property
                // of the enclosing list, now the top of the stack.
                let FrameKind::List(list) = &mut self.top().kind else {
                    unreachable!("an item end exposes its enclosing list");
                };
                let marker = if list.ordered {
                    let marker = format!("{}. ", list.next);
                    list.next += 1;
                    marker
                } else {
                    "- ".to_string()
                };
                let item = indent_continuation(body, &marker);
                list.items.push(item);
            }
            TagEnd::TableCell => {
                let cell = self.pop().body.finish();
                if let FrameKind::Table(table) = &mut self.top().kind {
                    table.current.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let FrameKind::Table(table) = &mut self.top().kind {
                    table.header = std::mem::take(&mut table.current);
                }
            }
            TagEnd::TableRow => {
                if let FrameKind::Table(table) = &mut self.top().kind {
                    let row = std::mem::take(&mut table.current);
                    table.rows.push(row);
                }
            }
            TagEnd::Table => {
                let frame = self.pop();
                let FrameKind::Table(table) = frame.kind else {
                    unreachable!("a table end closes a table frame");
                };
                self.append_block(&render_table(&table));
            }
            // A dropped subtree contributes nothing to its parent.
            TagEnd::FootnoteDefinition | TagEnd::HtmlBlock => {
                self.pop();
            }
            // Transparent inline tags and disabled constructs pushed no frame.
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::Link
            | TagEnd::Image
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    fn pop(&mut self) -> Frame {
        self.stack
            .pop()
            .expect("every end event closes a frame its start event opened")
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
/// alignment; body rows shorter than the header are padded with empty cells so
/// every row has the same width.
fn render_table(table: &TableState) -> String {
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
        let mut cells = row.clone();
        while cells.len() < columns {
            cells.push(String::new());
        }
        writer.syntax(&format!("{},\n", join_cells(&cells)));
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

#[cfg(test)]
mod tests {
    use super::*;

    use typst_syntax::{SyntaxKind, SyntaxNode};

    // =====================================================================
    // Paragraphs
    // =====================================================================

    #[test]
    fn empty_body_emits_nothing() {
        assert_eq!(emit(""), "");
    }

    #[test]
    fn single_paragraph_is_escaped_text_with_a_trailing_newline() {
        assert_eq!(emit("Just some prose."), "Just some prose\\.\n");
    }

    #[test]
    fn adjacent_paragraphs_are_separated_by_a_blank_line() {
        insta::assert_snapshot!(emit("First paragraph.\n\nSecond paragraph."));
    }

    #[test]
    fn soft_wrapped_lines_join_with_a_space() {
        // A soft break inside one paragraph is a single space; the paragraph
        // stays one line of output.
        assert_eq!(emit("one\ntwo\nthree"), "one two three\n");
    }

    #[test]
    fn hard_break_becomes_a_linebreak_call() {
        // Two trailing spaces force a hard break.
        assert_eq!(emit("one  \ntwo"), "one#linebreak()two\n");
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
        insta::assert_snapshot!(emit(input));
    }

    // =====================================================================
    // Code blocks
    // =====================================================================

    #[test]
    fn fenced_code_with_a_language_tag() {
        insta::assert_snapshot!(emit("```rust\nlet x = 1;\n```"));
    }

    #[test]
    fn fenced_code_without_a_language_tag() {
        insta::assert_snapshot!(emit("```\nplain code\n```"));
    }

    #[test]
    fn fence_info_string_keeps_only_the_first_token() {
        // GFM allows arbitrary words after the language (` ```rust ignore `);
        // Typst reads everything up to whitespace as the tag, so only the
        // first token survives.
        assert_eq!(
            emit("```rust ignore\nlet x = 1;\n```"),
            "```rust\nlet x = 1;\n```\n"
        );
    }

    #[test]
    fn fence_info_string_with_typst_active_characters_yields_no_tag() {
        // An info string that is not identifier-shaped would change how Typst
        // parses the raw block's first line; it is dropped entirely.
        assert_eq!(emit("```a]b#c\ncode\n```"), "```\ncode\n```\n");
    }

    #[test]
    fn code_containing_a_triple_backtick_grows_the_fence() {
        // The content documents a Markdown fence, so the emitted fence must be
        // one backtick longer than the run inside it.
        insta::assert_snapshot!(emit("````\na ``` fence inside\n````"));
    }

    #[test]
    fn indented_code_block() {
        insta::assert_snapshot!(emit("    indented code\n    second line"));
    }

    #[test]
    fn empty_code_block() {
        insta::assert_snapshot!(emit("```\n```"));
    }

    // =====================================================================
    // Bullet lists
    // =====================================================================

    #[test]
    fn flat_bullet_list() {
        assert_eq!(emit("- a\n- b\n- c"), "- a\n- b\n- c\n");
    }

    #[test]
    fn bullet_list_nested_three_deep() {
        insta::assert_snapshot!(emit("- outer\n  - middle\n    - inner"));
    }

    #[test]
    fn adjacent_bullet_lists_separated_by_a_paragraph() {
        // A paragraph between two lists keeps them as two separate blocks.
        insta::assert_snapshot!(emit("- a\n- b\n\ntext between\n\n- c\n- d"));
    }

    // =====================================================================
    // Ordered lists
    // =====================================================================

    #[test]
    fn ordered_list_starting_at_one() {
        assert_eq!(emit("1. a\n2. b\n3. c"), "1. a\n2. b\n3. c\n");
    }

    #[test]
    fn ordered_list_starting_at_six_keeps_the_real_numbers() {
        assert_eq!(
            emit("6. six\n7. seven\n8. eight"),
            "6. six\n7. seven\n8. eight\n"
        );
    }

    #[test]
    fn ordered_and_bullet_lists_nested_together() {
        insta::assert_snapshot!(emit("1. first\n   - bullet a\n   - bullet b\n2. second"));
    }

    // =====================================================================
    // Task lists
    // =====================================================================

    #[test]
    fn task_list_checked_and_unchecked() {
        insta::assert_snapshot!(emit("- [ ] todo\n- [x] done"));
    }

    // =====================================================================
    // List tightness
    // =====================================================================

    #[test]
    fn tight_list_items_sit_on_adjacent_lines() {
        assert_eq!(emit("- a\n- b"), "- a\n- b\n");
    }

    #[test]
    fn loose_list_items_are_separated_by_blank_lines() {
        insta::assert_snapshot!(emit("- a\n\n- b"));
    }

    #[test]
    fn loose_item_with_a_continuation_paragraph_indents_it() {
        insta::assert_snapshot!(emit(
            "- First paragraph.\n\n  Second paragraph.\n- Next item"
        ));
    }

    #[test]
    fn list_item_with_a_nested_code_block_indents_it() {
        insta::assert_snapshot!(emit("- item\n\n  ```\n  code\n  ```"));
    }

    // =====================================================================
    // Block quotes
    // =====================================================================

    #[test]
    fn quote_with_multiple_paragraphs() {
        insta::assert_snapshot!(emit("> First.\n>\n> Second."));
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
        insta::assert_snapshot!(emit(input));
    }

    #[test]
    fn callout_kind_is_lowercased() {
        // GFM matches the marker case-insensitively; the emitted kind is
        // always lowercase regardless of how the note wrote it.
        assert!(emit("> [!NoTe]\n> body").contains("#callout(kind: \"note\")["));
    }

    #[test]
    fn unknown_callout_kind_falls_back_to_a_plain_quote() {
        // An unrecognized marker is not a callout: it renders as an ordinary
        // quote and the `[!FOO]` text survives escaped in the body.
        insta::assert_snapshot!(emit("> [!FOO]\n> body"));
    }

    // =====================================================================
    // Tables
    // =====================================================================

    #[test]
    fn basic_table() {
        insta::assert_snapshot!(emit("| H1 | H2 |\n| --- | --- |\n| a | b |\n| c | d |"));
    }

    #[test]
    fn table_with_every_alignment() {
        insta::assert_snapshot!(emit(
            "| L | C | R |\n| :--- | :---: | ---: |\n| a | b | c |"
        ));
    }

    #[test]
    fn table_with_default_alignment() {
        // A header separator with no colons leaves the column alignment unset,
        // which maps to Typst's `auto`.
        insta::assert_snapshot!(emit("| H1 | H2 |\n| --- | --- |\n| a | b |"));
    }

    #[test]
    fn table_body_row_shorter_than_the_header_is_padded() {
        // The second body row omits its last cell; the emitter pads it so
        // every row matches the header's column count.
        insta::assert_snapshot!(emit(
            "| H1 | H2 | H3 |\n| --- | --- | --- |\n| a | b | c |\n| d |"
        ));
    }

    #[test]
    fn table_cells_escape_markup_active_text() {
        insta::assert_snapshot!(emit("| Name | Note |\n| --- | --- |\n| a*b | c_d |"));
    }

    // =====================================================================
    // Thematic breaks
    // =====================================================================

    #[test]
    fn thematic_break_between_paragraphs() {
        insta::assert_snapshot!(emit("before\n\n---\n\nafter"));
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
        let emitted = emit(input);

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
        let emitted = emit("First paragraph of prose.\n\nSecond paragraph of prose.");

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
