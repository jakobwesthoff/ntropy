// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The Markdown-to-Typst emitter: a `pulldown-cmark` event loop that turns a
//! note body into a complete Typst body fragment
//! (`docs/design/typst-engine.md`, "Element mapping" and "Writer and
//! context model").
//!
//! # What the emitter covers
//!
//! Block constructs: paragraphs, headings, fenced and indented code blocks,
//! bullet and ordered lists (including task lists, nesting, and loose-list
//! continuation blocks), block quotes, GFM callouts, tables, and thematic
//! breaks.
//!
//! Inline constructs, all emitted as Typst function calls so that no
//! user-derived character is ever load-bearing markup:
//!
//! - Emphasis, strong, and strikethrough wrap their content in `#emph[…]`,
//!   `#strong[…]`, and `#strike[…]`; the wrappers nest.
//! - Inline code becomes `#raw("…")` with string-literal escaping.
//! - Soft breaks map to a space and hard breaks to `#linebreak()`.
//! - Links check the note-link table first: a Link event whose byte span
//!   equals a [`ResolvedLink::range`] is a note link. A resolved note link
//!   (the target has a title) renders as `#emph[Title]` and its inner events
//!   are dropped; an unresolved note link drops only the wrapper and
//!   re-emits its inner markup, so formatting in the display text survives.
//!   Every other link, along with URL and email autolinks, renders as
//!   `#link("target")[label]`.
//! - Autolinks are detected with `linkify` over `Text` events that are not
//!   inside a link label or raw content. A scheme URL keeps its target; a
//!   `www.` URL is prefixed with `https://`; an email becomes a `mailto:`
//!   link. Known limitation: a URL split across text events (by an entity or
//!   adjacent markup) is not detected, because detection is per event.
//! - Images collect their alt subtree as plain text. A local path becomes
//!   `#image("path")`; a remote `http(s)` image cannot be embedded by the
//!   offline compiler, so it degrades to `#link("url")[alt-or-url]` and
//!   raises a [`Warning`].
//! - Footnotes are assembled in two passes. Definitions (which may hold
//!   block content and may appear before or after their references) are
//!   collected during the walk; each reference emits a unique placeholder
//!   token that is replaced with the definition's content once the walk is
//!   done. A reference to an undefined footnote renders as its literal
//!   source text (GFM behaviour); a definition nobody references produces no
//!   output.
//! - Raw HTML — both `Event::Html` blocks and `Event::InlineHtml` fragments
//!   — is dropped and raises a [`Warning`] naming the dropped fragment.
//!
//! Math is off, so `$` is ordinary escaped text.
//!
//! # Structure
//!
//! The loop carries a stack of [`Frame`]s, one per open container. A frame is
//! the natural home for the facts `pulldown-cmark` 0.13 only exposes at
//! `Start` (fence language, list ordinality, table alignments, a link's kind)
//! and for content a container can only shape once complete (a code fence
//! sized from its content, a table laid out from its rows, an inline wrapper
//! sized around its children). Inline text and finished children accumulate
//! in the frame's [`TypstWriter`]; every character of user text passes
//! through one of the writer's escaped channels, and finished children are
//! spliced back through the unescaped `syntax` channel. When a container
//! closes, its frame produces one string the parent incorporates: block
//! frames separate with a blank line, inline frames concatenate in place.

use std::collections::HashMap;

use linkify::{LinkFinder, LinkKind};
use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd,
};

use super::writer::{self, TypstWriter};
use crate::render::ResolvedLink;

// =========================================================
// Public entry
// =========================================================

/// A non-fatal problem the emitter surfaces to the engine, which forwards it
/// to `RenderContext::warn`. Raised for content the emitter cannot faithfully
/// carry into the artifact: a remote image the offline compiler cannot embed,
/// and dropped raw HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub message: String,
}

impl Warning {
    fn new(message: impl Into<String>) -> Self {
        Warning {
            message: message.into(),
        }
    }
}

/// Convert a note body to a Typst body fragment, resolving note links against
/// `links` (ADR 0028).
///
/// The returned string is the converted body alone; document assembly
/// (prelude, template application) lives in the engine layer, which also
/// forwards the returned warnings to the host.
pub fn emit(body: &str, links: &[ResolvedLink]) -> (String, Vec<Warning>) {
    // GitHub's rendered surface: tables, strikethrough, task lists, and
    // footnotes, with `ENABLE_GFM` supplying the callout kinds on block
    // quotes. Math stays off by omission, so `$` never gains meaning.
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;

    let mut emitter = Emitter::new(links);
    // The offset iterator carries each event's byte span. The span identifies
    // note links: a Link event's span is matched against `ResolvedLink::range`
    // (both index the same body bytes) to distinguish note links from ordinary
    // ones.
    for (event, span) in Parser::new_ext(body, options).into_offset_iter() {
        emitter.handle(event, span);
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
    /// An emphasis/strong/strikethrough span. Inner markup accumulates in the
    /// frame body; the closing tag wraps it in the matching function call.
    Styled(InlineStyle),
    /// A link. Its inner markup accumulates in the frame body; the closing tag
    /// materializes the link per its resolved kind.
    Link(LinkEmit),
    /// An image. Its alt subtree is collected as plain text (markup flattened),
    /// then rendered as `#image` (local) or a degraded `#link` (remote).
    Image {
        dest: String,
        alt: String,
    },
    /// A footnote definition's block content. Buffered here and diverted into
    /// the definitions map at its `End`, never spliced into the surrounding
    /// document.
    FootnoteDefinition {
        label: String,
    },
    /// A raw HTML block. Its verbatim text accumulates so the drop `Warning`
    /// can name the fragment; nothing reaches the output.
    HtmlBlock {
        content: String,
    },
}

/// The three inline span stylings the emitter wraps as function calls.
enum InlineStyle {
    Emph,
    Strong,
    Strike,
}

/// How a link materializes at its `End`, decided at `Start` from the note-link
/// table and the link's destination.
enum LinkEmit {
    /// Not a note link: `#link("dest")[inner]`, `dest` string-literal escaped.
    Ordinary { dest: String },
    /// A note link whose target resolves: `#emph[Title]`, inner events dropped.
    ResolvedNote { title: String },
    /// A note link whose target is dangling: the wrapper is dropped and the
    /// inner markup re-emitted, so formatting in the display text survives.
    UnresolvedNote,
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

struct Emitter<'a> {
    stack: Vec<Frame>,
    /// The note-link table, matched by byte range against Link events.
    links: &'a [ResolvedLink],
    /// Warnings accumulated during the walk, returned alongside the body.
    warnings: Vec<Warning>,
    /// URL/email detector for autolinking `Text` events. Configured once.
    finder: LinkFinder,
    /// Footnote definitions collected during the walk, keyed by label. Filled
    /// wherever a definition appears; consumed when references are patched.
    footnote_definitions: HashMap<String, String>,
    /// The labels every footnote reference used, so patching visits exactly
    /// the referenced ones (undefined included, unreferenced excluded).
    footnote_references: Vec<String>,
    /// A per-run random token wrapping footnote-reference placeholders. See
    /// [`Emitter::footnote_placeholder`] for the collision argument.
    nonce: String,
    /// Depth of open links. A positive depth suppresses autolinking, because
    /// GFM never autolinks inside a link's label.
    link_depth: usize,
    /// Depth of open images. A positive depth flattens all inner inline markup
    /// into the current image's alt text.
    image_depth: usize,
}

impl<'a> Emitter<'a> {
    fn new(links: &'a [ResolvedLink]) -> Self {
        // GFM extended autolinks cover scheme URLs, `www.` URLs, and emails.
        // `linkify` finds scheme URLs and emails out of the box; enabling
        // `www.` needs `url_must_have_scheme(false)`, which also matches bare
        // dotted words like `report.txt`. Those false positives are filtered
        // at emission (see `write_autolinked`): only real schemes, `www.`
        // prefixes, and emails become links.
        let mut finder = LinkFinder::new();
        finder.kinds(&[LinkKind::Url, LinkKind::Email]);
        finder.url_must_have_scheme(false);

        Emitter {
            stack: vec![Frame {
                kind: FrameKind::Document,
                body: TypstWriter::new(),
                nonempty: false,
            }],
            links,
            warnings: Vec::new(),
            finder,
            footnote_definitions: HashMap::new(),
            footnote_references: Vec::new(),
            // A fresh ULID carries 80 random bits. Wrapping the reference
            // placeholder token in this nonce makes the token unequal to any
            // substring the note can produce: the note author cannot embed a
            // value that is generated randomly, per run, after the note is
            // written. NUL is added as a second guard (Typst markup never
            // writes it), though the randomness is what makes the token safe;
            // pulldown-cmark 0.13 does not strip NUL from text, so NUL alone
            // would not suffice. Every token is substituted out before `emit`
            // returns, so the artifact is deterministic and nonce-free.
            nonce: format!("\u{0}ntropy-footnote-{}\u{0}", ulid::Ulid::new()),
            link_depth: 0,
            image_depth: 0,
        }
    }

    /// The placeholder emitted at a footnote reference and patched at the end
    /// of the walk. Identical for repeated references to one label, so a
    /// single substitution rule inlines the definition at every site.
    fn footnote_placeholder(&self, label: &str) -> String {
        format!("{n}{label}{n}", n = self.nonce)
    }

    fn finish(mut self) -> (String, Vec<Warning>) {
        let document = self
            .stack
            .pop()
            .expect("the document frame is pushed at construction and never popped");
        let mut body = document.body.finish();

        // Patch every footnote reference. A defined reference inlines the
        // definition's content inside `#footnote[…]`; an undefined one renders
        // as its literal source text (GFM). The substitutions are applied
        // repeatedly because a definition may itself contain references, whose
        // placeholders only appear once the enclosing definition is inlined;
        // the pass count bounds any self-referential cycle.
        let substitutions: Vec<(String, String)> = self
            .footnote_references
            .iter()
            .map(|label| {
                let placeholder = self.footnote_placeholder(label);
                let replacement = match self.footnote_definitions.get(label) {
                    Some(content) => format!("#footnote[{}]", content.trim_end_matches('\n')),
                    None => escape_markup(&format!("[^{label}]")),
                };
                (placeholder, replacement)
            })
            .collect();

        for _ in 0..=substitutions.len() {
            let mut changed = false;
            for (placeholder, replacement) in &substitutions {
                if body.contains(placeholder.as_str()) {
                    body = body.replace(placeholder, replacement);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        (body, self.warnings)
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

    fn handle(&mut self, event: Event, span: std::ops::Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag, span),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.code(&code),
            // Math is disabled, so these never arrive; routing their content
            // as text keeps the match total without a panic path.
            Event::InlineMath(m) | Event::DisplayMath(m) => self.text(&m),
            Event::SoftBreak => self.inline_break(" "),
            Event::HardBreak => self.inline_break("#linebreak()"),
            Event::Rule => self.append_block("#line(length: 100%)\n"),
            Event::TaskListMarker(checked) => {
                self.inline_syntax(if checked { "☑ " } else { "☐ " });
            }
            Event::FootnoteReference(label) => self.footnote_reference(&label),
            // A raw HTML block's text arrives as `Html` events between its
            // `Start`/`End`; collect it so the drop warning can name it.
            Event::Html(html) => self.html_block_text(&html),
            Event::InlineHtml(html) => self.inline_html(&html),
        }
    }

    // -----------------------------------------------------
    // Text and inline routing
    // -----------------------------------------------------

    /// Route user text to the correct channel for the current container: an
    /// open image collects it as plain alt text; a code block keeps it
    /// verbatim; everywhere else it is escaped markup, autolinked unless the
    /// text sits inside a link label.
    fn text(&mut self, text: &str) {
        if self.image_depth > 0 {
            self.push_alt(text);
            return;
        }
        let in_link = self.link_depth > 0;
        // Borrow the finder and the top frame as disjoint fields so the
        // autolinker can write into the frame while reading the finder.
        let finder = &self.finder;
        let frame = self
            .stack
            .last_mut()
            .expect("the document frame keeps the stack non-empty");
        match &mut frame.kind {
            FrameKind::CodeBlock { content, .. } => content.push_str(text),
            FrameKind::HtmlBlock { .. } => {}
            _ => {
                if in_link {
                    frame.body.markup_text(text);
                } else {
                    write_autolinked(finder, &mut frame.body, text);
                }
                frame.nonempty = true;
            }
        }
    }

    /// Inline code: `#raw("…")` with string-literal escaping. Inside an image
    /// alt it degrades to the code's plain text, matching the flatten policy.
    fn code(&mut self, code: &str) {
        if self.image_depth > 0 {
            self.push_alt(code);
            return;
        }
        let mut writer = TypstWriter::new();
        writer.syntax("#raw(\"");
        writer.string_literal(code);
        writer.syntax("\")");
        self.append_inline(&writer.finish());
    }

    /// A soft or hard break. Inside an image alt both collapse to a space in
    /// the flattened text; elsewhere the caller's syntax is emitted inline.
    fn inline_break(&mut self, syntax: &str) {
        if self.image_depth > 0 {
            self.push_alt(" ");
            return;
        }
        self.inline_syntax(syntax);
    }

    /// Emitter-owned inline markup (the task-list box, break syntax), routed so
    /// it never lands in a code block's verbatim content or a dropped subtree.
    fn inline_syntax(&mut self, s: &str) {
        let frame = self.top();
        match &frame.kind {
            FrameKind::CodeBlock { .. } | FrameKind::HtmlBlock { .. } => {}
            _ => {
                frame.body.syntax(s);
                frame.nonempty = true;
            }
        }
    }

    /// Splice already-escaped inline content into the current container with no
    /// separation, marking it non-empty. Used by every inline construct as it
    /// closes.
    fn append_inline(&mut self, s: &str) {
        let frame = self.top();
        frame.body.syntax(s);
        frame.nonempty = true;
    }

    /// Append plain text to the innermost open image's alt buffer.
    fn push_alt(&mut self, text: &str) {
        if let FrameKind::Image { alt, .. } = &mut self.top().kind {
            alt.push_str(text);
        }
    }

    /// Emit a footnote reference: record the label and drop a placeholder that
    /// `finish` patches once every definition is known. Inside an image alt a
    /// reference is meaningless and is dropped.
    fn footnote_reference(&mut self, label: &str) {
        if self.image_depth > 0 {
            return;
        }
        self.footnote_references.push(label.to_string());
        let placeholder = self.footnote_placeholder(label);
        self.append_inline(&placeholder);
    }

    /// Collect a raw HTML block's verbatim text so its drop warning can name
    /// the fragment.
    fn html_block_text(&mut self, html: &str) {
        if let FrameKind::HtmlBlock { content } = &mut self.top().kind {
            content.push_str(html);
        }
    }

    /// Drop an inline raw HTML fragment, warning with its content. Inside an
    /// image alt the fragment is dropped silently, as alt is plain text.
    fn inline_html(&mut self, html: &str) {
        if self.image_depth > 0 {
            return;
        }
        self.warnings.push(Warning::new(format!(
            "dropped raw HTML: {}",
            truncate_fragment(html)
        )));
    }

    // -----------------------------------------------------
    // Start events
    // -----------------------------------------------------

    fn start(&mut self, tag: Tag, span: std::ops::Range<usize>) {
        // Inside an image alt every construct flattens to its text; only a
        // nested image bumps the depth so the matching `End` is balanced.
        if self.image_depth > 0 {
            if matches!(tag, Tag::Image { .. }) {
                self.image_depth += 1;
            }
            return;
        }
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
            // A definition's block content is diverted to the definitions map
            // at its `End`, never spliced into the surrounding document.
            Tag::FootnoteDefinition(label) => {
                self.push(FrameKind::FootnoteDefinition {
                    label: label.to_string(),
                });
            }
            Tag::HtmlBlock => self.push(FrameKind::HtmlBlock {
                content: String::new(),
            }),
            Tag::Emphasis => self.push(FrameKind::Styled(InlineStyle::Emph)),
            Tag::Strong => self.push(FrameKind::Styled(InlineStyle::Strong)),
            Tag::Strikethrough => self.push(FrameKind::Styled(InlineStyle::Strike)),
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                // An angle-bracket email autolink (`<user@host>`) arrives with
                // the bare address as its destination; a PDF viewer needs the
                // `mailto:` scheme to act on it, exactly as GFM renders it.
                let dest = if link_type == LinkType::Email {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.to_string()
                };
                let emit = self.classify_link(&dest, span);
                self.push(FrameKind::Link(emit));
                self.link_depth += 1;
            }
            Tag::Image { dest_url, .. } => {
                self.push(FrameKind::Image {
                    dest: dest_url.to_string(),
                    alt: String::new(),
                });
                self.image_depth += 1;
            }
            // Not enabled in the parser options, so never produced. Super- and
            // subscript stay transparent: their inner text flows on unwrapped.
            Tag::Superscript
            | Tag::Subscript
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {}
        }
    }

    /// Decide how a link materializes. A byte span equal to a note link's
    /// range makes it a note link, resolved or dangling by the presence of a
    /// target title; any other span is an ordinary link.
    fn classify_link(&self, dest: &str, span: std::ops::Range<usize>) -> LinkEmit {
        match self.links.iter().find(|link| link.range == span) {
            Some(link) => match &link.target_title {
                Some(title) => LinkEmit::ResolvedNote {
                    title: title.clone(),
                },
                None => LinkEmit::UnresolvedNote,
            },
            None => LinkEmit::Ordinary {
                dest: dest.to_string(),
            },
        }
    }

    // -----------------------------------------------------
    // End events
    // -----------------------------------------------------

    fn end(&mut self, tag: TagEnd) {
        // Inside an image alt only the image's own close matters; when it
        // brings the depth back to zero the frame is finalized.
        if self.image_depth > 0 {
            if matches!(tag, TagEnd::Image) {
                self.image_depth -= 1;
                if self.image_depth == 0 {
                    self.finish_image();
                }
            }
            return;
        }
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
            TagEnd::FootnoteDefinition => {
                let frame = self.pop();
                let FrameKind::FootnoteDefinition { label } = frame.kind else {
                    unreachable!("a footnote-definition end closes its frame");
                };
                // Later definitions with a repeated label overwrite earlier
                // ones, matching pulldown-cmark's own last-wins resolution.
                self.footnote_definitions.insert(label, frame.body.finish());
            }
            TagEnd::HtmlBlock => {
                let frame = self.pop();
                let FrameKind::HtmlBlock { content } = frame.kind else {
                    unreachable!("an HTML-block end closes its frame");
                };
                self.warnings.push(Warning::new(format!(
                    "dropped raw HTML: {}",
                    truncate_fragment(&content)
                )));
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                let frame = self.pop();
                let FrameKind::Styled(style) = frame.kind else {
                    unreachable!("an inline-style end closes a styled frame");
                };
                let inner = frame.body.finish();
                self.append_inline(&wrap_styled(&style, &inner));
            }
            TagEnd::Link => {
                self.link_depth -= 1;
                let frame = self.pop();
                let FrameKind::Link(emit) = frame.kind else {
                    unreachable!("a link end closes a link frame");
                };
                let inner = frame.body.finish();
                let out = match emit {
                    LinkEmit::Ordinary { dest } => wrap_link(&dest, &inner),
                    // The inner events were collected but a resolved note link
                    // shows the target's title instead, so `inner` is dropped.
                    LinkEmit::ResolvedNote { title } => wrap_emph_text(&title),
                    LinkEmit::UnresolvedNote => inner,
                };
                self.append_inline(&out);
            }
            // Image is finalized in the depth-guarded branch above; disabled
            // and transparent constructs pushed no frame.
            TagEnd::Image
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    /// Render the innermost image and splice it into its parent. A local path
    /// becomes `#image("path")`; a remote one cannot be embedded offline and
    /// degrades to a link plus a warning.
    fn finish_image(&mut self) {
        let frame = self.pop();
        let FrameKind::Image { dest, alt } = frame.kind else {
            unreachable!("an image end closes an image frame");
        };

        let lowered = dest.to_ascii_lowercase();
        let remote = lowered.starts_with("http://") || lowered.starts_with("https://");

        let mut writer = TypstWriter::new();
        if remote {
            // The label falls back to the URL when the alt is empty, so the
            // degraded link is never blank.
            let label = if alt.is_empty() { dest.as_str() } else { &alt };
            writer.syntax("#link(\"");
            writer.string_literal(&dest);
            writer.syntax("\")[");
            writer.markup_text(label);
            writer.syntax("]");
            self.warnings.push(Warning::new(format!(
                "remote image cannot be embedded; linked instead: {dest}"
            )));
        } else {
            writer.syntax("#image(\"");
            writer.string_literal(&dest);
            writer.syntax("\")");
        }
        self.append_inline(&writer.finish());
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

// =========================================================
// Inline rendering
// =========================================================

/// Wrap already-escaped inner markup in the styling function for `style`.
fn wrap_styled(style: &InlineStyle, inner: &str) -> String {
    let call = match style {
        InlineStyle::Emph => "emph",
        InlineStyle::Strong => "strong",
        InlineStyle::Strike => "strike",
    };
    format!("#{call}[{inner}]")
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

/// An `#emph[Title]` call with `title` escaped as markup text, used for a
/// resolved note link.
fn wrap_emph_text(title: &str) -> String {
    let mut writer = TypstWriter::new();
    writer.syntax("#emph[");
    writer.markup_text(title);
    writer.syntax("]");
    writer.finish()
}

/// Escape a string as markup text in isolation, for content assembled outside
/// a frame's writer (a footnote reference's literal fallback).
fn escape_markup(text: &str) -> String {
    let mut writer = TypstWriter::new();
    writer.markup_text(text);
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

/// Write `text` to `writer`, turning autolinkable spans into `#link` calls and
/// escaping the rest as markup.
///
/// `linkify` is configured (in [`Emitter::new`]) to also surface scheme-less
/// URLs, which include `www.` hosts but also bare dotted words. The filter
/// here keeps only spans that carry meaning as links: any real scheme, a
/// `www.` host (GFM prefixes it with `https://`), and emails (rendered as
/// `mailto:` links). A scheme-less non-`www.` span is not a link and flows
/// through as text.
fn write_autolinked(finder: &LinkFinder, writer: &mut TypstWriter, text: &str) {
    for span in finder.spans(text) {
        let s = span.as_str();
        let link = match span.kind() {
            Some(LinkKind::Url) if s.contains("://") => Some((s.to_string(), s)),
            Some(LinkKind::Url) if starts_with_ascii_ci(s, "www.") => {
                Some((format!("https://{s}"), s))
            }
            Some(LinkKind::Email) => Some((format!("mailto:{s}"), s)),
            // A scheme-less, non-`www.` match (`report.txt`) is not a link.
            _ => None,
        };
        match link {
            Some((target, label)) => {
                writer.syntax("#link(\"");
                writer.string_literal(&target);
                writer.syntax("\")[");
                writer.markup_text(label);
                writer.syntax("]");
            }
            None => writer.markup_text(s),
        }
    }
}

/// Case-insensitive ASCII prefix test, for the `www.` autolink rule. Compares
/// bytes so an IRI match whose fourth byte falls inside a multi-byte character
/// cannot panic a string slice.
fn starts_with_ascii_ci(haystack: &str, prefix: &str) -> bool {
    let (haystack, prefix) = (haystack.as_bytes(), prefix.as_bytes());
    haystack.len() >= prefix.len() && haystack[..prefix.len()].eq_ignore_ascii_case(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Range;

    use typst_syntax::{SyntaxKind, SyntaxNode};

    /// Emit a body with no note-link table, keeping the string alone. Most
    /// tests exercise constructs the note-link table does not touch.
    fn body(input: &str) -> String {
        emit(input, &[]).0
    }

    /// Emit and keep only the warnings, for the raw-HTML and remote-image
    /// cases whose contract is the warning rather than the body.
    fn warnings(input: &str) -> Vec<Warning> {
        emit(input, &[]).1
    }

    /// Build a note-link table entry. The range must equal the Link event's
    /// byte span in `input` for the emitter to treat it as a note link.
    fn note_link(range: Range<usize>, target_title: Option<&str>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: crate::id::Id::from_timestamp_ms(0),
            target_title: target_title.map(str::to_string),
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
    fn resolved_note_link_becomes_emphasized_title() {
        // `[my note](abc123)` spans bytes 4..21; a matching resolved entry
        // replaces the whole link with its target's current title.
        let input = "See [my note](abc123) end.";
        let links = [note_link(4..21, Some("Target Title"))];
        assert_eq!(emit(input, &links).0, "See #emph[Target Title] end\\.\n");
    }

    #[test]
    fn resolved_note_link_discards_inner_markup_label() {
        // The display text carries markup, but a resolved note link shows the
        // target's title instead, so the inner `#strong` never appears.
        let input = "[**bold** display](id)";
        let links = [note_link(0..22, Some("The Title"))];
        assert_eq!(emit(input, &links).0, "#emph[The Title]\n");
    }

    #[test]
    fn resolved_note_link_title_escapes_markup_active_characters() {
        let input = "[x](id)";
        let links = [note_link(0..7, Some("a*b [c] #d"))];
        assert_eq!(emit(input, &links).0, "#emph[a\\*b \\[c\\] \\#d]\n");
    }

    #[test]
    fn unresolved_note_link_re_emits_its_markup_label() {
        // A dangling note link drops the wrapper and keeps the display markup.
        let input = "[*display* text](id)";
        let links = [note_link(0..20, None)];
        assert_eq!(emit(input, &links).0, "#emph[display] text\n");
    }

    #[test]
    fn resolved_link_whose_range_matches_no_event_is_ignored() {
        // The note text already renders literally; a stray table entry that
        // matches no Link event changes nothing.
        let input = "just prose";
        let links = [note_link(0..4, Some("Title"))];
        assert_eq!(emit(input, &links).0, "just prose\n");
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
        assert_eq!(body("![alt text](pics/x.png)"), "#image(\"pics/x.png\")\n");
    }

    #[test]
    fn image_with_empty_alt() {
        assert_eq!(body("![](x.png)"), "#image(\"x.png\")\n");
    }

    #[test]
    fn image_alt_with_markup_flattens_to_plain_text() {
        // Only the remote form emits the alt, and it emits the flattened text
        // with the emphasis markers gone.
        let (out, warnings) = emit("![*bold* and `code`](https://h/x.png)", &[]);
        assert_eq!(out, "#link(\"https://h/x.png\")[bold and code]\n");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn remote_image_degrades_to_a_link_with_a_warning() {
        let (out, warnings) = emit("![caption](http://host/pic.png)", &[]);
        assert_eq!(out, "#link(\"http://host/pic.png\")[caption]\n");
        assert_eq!(
            warnings[0].message,
            "remote image cannot be embedded; linked instead: http://host/pic.png"
        );
    }

    #[test]
    fn remote_image_with_empty_alt_labels_with_the_url() {
        let (out, _) = emit("![](https://host/pic.png)", &[]);
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
            "#link(\"https://target\")[#image(\"local.png\")]\n"
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
    fn www_prefix_test_is_byte_safe_across_multibyte_boundaries() {
        // A short multi-byte string must not panic the prefix test, which
        // guards the `www.` autolink rule against `str` slicing.
        assert!(!starts_with_ascii_ci("wwü", "www."));
        assert!(starts_with_ascii_ci("WWW.example.com", "www."));
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
        let (out, warnings) = emit("<div class=\"box\">\ncontent\n</div>", &[]);
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
        let (out, warnings) = emit("text <span>x</span> more", &[]);
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
            note_link(note_span, Some("Resolved Title")),
            note_link(dangling_span, None),
        ];
        let (out, warnings) = emit(input, &links);
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
            note_link(note_span, Some("Resolved Title")),
            note_link(dangling_span, None),
        ];
        let (out, _) = emit(input, &links);

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
