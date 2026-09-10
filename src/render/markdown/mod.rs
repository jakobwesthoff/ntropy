// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The shared Markdown walk: one `pulldown-cmark` event loop that turns a
//! note body into an output fragment through an [`Output`] implementation
//! (ADR 0049, `docs/design/html-engine.md`).
//!
//! The split is by ownership. The walk owns every fact that is a property of
//! the Markdown structure and of ntropy's link model, independent of what is
//! being produced:
//!
//! - the stack of open containers and the block/inline splice rules,
//! - note-link classification: a Link event whose byte span equals a
//!   [`ResolvedLink::range`] is a note link, resolved or dangling by the
//!   presence of a target (ADR 0028),
//! - autolink detection with `linkify` over `Text` events outside link labels
//!   and raw content, with GFM's rules for which spans count (a real scheme, a
//!   `www.` host gaining `https://`, an email gaining `mailto:`); a URL split
//!   across text events is not detected, because detection is per event,
//! - the `mailto:` scheme on angle-bracket email autolinks,
//! - loose-list detection, table row assembly and padding,
//! - image alt flattening: every construct inside an image collapses to its
//!   plain text,
//! - footnotes in two passes: definitions are collected wherever they appear,
//!   each reference leaves a placeholder that is patched once the walk is
//!   done; an undefined reference renders as its literal source text (GFM), an
//!   unreferenced definition produces nothing.
//!
//! An [`Output`] owns the markup and the escaping. It receives user text and
//! already-rendered children and returns rendered strings; it never sees a
//! parser event. Math is off in the parser options, so `$` reaches the output
//! as ordinary text.
//!
//! # Structure
//!
//! The loop carries a stack of [`Frame`]s, one per open container. A frame is
//! the natural home for the facts `pulldown-cmark` 0.13 only exposes at
//! `Start` (fence info, list ordinality, table alignments, a link's kind) and
//! for content a container can only shape once complete. Rendered inline
//! content and finished child blocks accumulate in the frame's body; when a
//! container closes, the output turns the body into one string the parent
//! incorporates: block frames separate with a blank line, inline frames
//! concatenate in place.

use std::collections::HashMap;

use linkify::{LinkFinder, LinkKind};
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};

pub use pulldown_cmark::{Alignment, BlockQuoteKind, HeadingLevel};

use crate::id::Id;
use crate::render::ResolvedLink;

// =========================================================
// The output contract
// =========================================================

/// A non-fatal problem surfaced to the engine, which forwards it to
/// `RenderContext::warn`. Raised for content an output cannot faithfully carry
/// into its artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub message: String,
}

impl Warning {
    pub fn new(message: impl Into<String>) -> Self {
        Warning {
            message: message.into(),
        }
    }
}

/// The three inline span stylings Markdown distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineStyle {
    Emph,
    Strong,
    Strike,
}

/// A finished list. Items are the rendered bodies of each item, in order,
/// exactly as their frames accumulated them (a block item's body ends in a
/// newline, a tight item's does not).
pub struct List {
    /// The first ordinal of an ordered list; `None` for a bullet list.
    pub start: Option<u64>,
    /// A blank line between two items, or any item wrapped in a paragraph,
    /// makes the whole list loose.
    pub loose: bool,
    pub items: Vec<String>,
}

/// A heading's level, its id, and its plain text.
///
/// The id follows GitHub's rule over the plain text: lowercased, whitespace
/// becomes a hyphen, every character that is not a letter, a digit, a hyphen,
/// or an underscore is dropped, and a repeated id within one body gets `-1`,
/// `-2`, and so on. Anchor links written GitHub-style in a note therefore
/// resolve unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: HeadingLevel,
    pub id: String,
    /// The heading's text with inline markup flattened away, as an outline
    /// shows it.
    pub text: String,
}

/// A finished table. The header fixes the column count; every body row is
/// padded with empty cells to that width, so an output can rely on the table
/// being rectangular.
pub struct Table {
    pub alignments: Vec<Alignment>,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// What an output produces for each construct. Inline methods return content
/// the walk splices into the enclosing container in place; block methods
/// return content the walk separates from its siblings with a blank line, so a
/// block's string carries its own trailing newline.
///
/// `inner`, `body`, `content` arguments are already rendered by earlier calls
/// on the same output and must be spliced unescaped. `text`, `code`, `dest`,
/// `alt`, `title`, `slug`, `info`, `html` arguments are user-derived and the
/// output escapes them for the context it places them in.
pub trait Output {
    /// User text in running content.
    fn text(&mut self, text: &str) -> String;

    /// Inline code.
    fn inline_code(&mut self, code: &str) -> String;

    fn soft_break(&mut self) -> String;

    fn hard_break(&mut self) -> String;

    /// The checkbox that leads a task-list item.
    fn task_marker(&mut self, checked: bool) -> String;

    fn styled(&mut self, style: InlineStyle, inner: &str) -> String;

    /// A link that is not a note link. `dest` already carries its scheme.
    fn link(&mut self, dest: &str, inner: &str) -> String;

    /// A URL or email the walk detected in running text. `target` carries the
    /// scheme GFM adds; `label` is the text as written.
    fn autolink(&mut self, target: &str, label: &str) -> String {
        let label = self.text(label);
        self.link(target, &label)
    }

    /// A note link whose target resolves: the target's id, its current title,
    /// and the slug its default artifact is named after (ADR 0044). The
    /// link's own display text is not offered, since the artifact shows the
    /// title.
    fn note_link(&mut self, id: Id, title: &str, slug: &str) -> String;

    /// An image. `alt` is the flattened plain text of the alt subtree. An
    /// output that cannot carry the image records why in `warnings`.
    fn image(&mut self, dest: &str, alt: &str, warnings: &mut Vec<Warning>) -> String;

    /// A footnote reference, given the definition's rendered block content.
    /// Called once per label, in first-reference order; the result replaces
    /// every reference to that label. An output either inlines the content
    /// here or keeps it for the section it appends in [`Output::finish`].
    fn footnote(&mut self, content: &str) -> String;

    /// A raw HTML fragment inside running content. `None` drops it; an output
    /// that drops records why in `warnings`.
    fn inline_html(&mut self, html: &str, warnings: &mut Vec<Warning>) -> Option<String>;

    fn paragraph(&mut self, body: &str) -> String;

    /// A heading. `body` is the rendered inline content; `heading` carries
    /// the level, the id, and the plain text.
    fn heading(&mut self, heading: &Heading, body: &str) -> String;

    /// A block quote, or a GFM callout when `kind` is present.
    fn quote(&mut self, kind: Option<BlockQuoteKind>, body: &str) -> String;

    /// A code block. `info` is the fence's info string as written (`None` for
    /// an indented block); the output decides what of it becomes a language
    /// tag. `content` is verbatim.
    fn code_block(&mut self, info: Option<&str>, content: &str) -> String;

    fn list(&mut self, list: &List) -> String;

    fn table(&mut self, table: &Table) -> String;

    /// A thematic break.
    fn rule(&mut self) -> String;

    /// A raw HTML block. `None` drops it; an output that drops records why in
    /// `warnings`.
    fn html_block(&mut self, html: &str, warnings: &mut Vec<Warning>) -> Option<String>;

    /// The output's last word over the rendered document body, called once
    /// after the last event and before footnote placeholders are patched.
    /// Content appended here may therefore itself contain footnote
    /// references (a footnote section built from definitions, say) and still
    /// gets its markers.
    fn finish(&mut self, body: String) -> String {
        body
    }
}

// =========================================================
// Public entry
// =========================================================

/// Walk `body` and render it through `output`, resolving note links against
/// `links` (ADR 0028).
///
/// The returned string is the converted body alone; document assembly is the
/// engine's job, and so is forwarding the warnings to the host.
pub fn walk<O: Output>(
    body: &str,
    links: &[ResolvedLink],
    output: &mut O,
) -> (String, Vec<Warning>) {
    // GitHub's rendered surface: tables, strikethrough, task lists, and
    // footnotes, with `ENABLE_GFM` supplying the callout kinds on block
    // quotes. Math stays off by omission, so `$` never gains meaning.
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;

    let mut walker = Walker::new(links, output);
    // The offset iterator carries each event's byte span. The span identifies
    // note links: a Link event's span is matched against `ResolvedLink::range`
    // (both index the same body bytes) to distinguish note links from ordinary
    // ones.
    for (event, span) in Parser::new_ext(body, options).into_offset_iter() {
        walker.handle(event, span);
    }
    walker.finish()
}

// =========================================================
// The structural stack
// =========================================================

/// One open container. `body` accumulates the container's rendered inline
/// content and its already-rendered child blocks; `nonempty` records whether
/// anything has been written yet, which decides block separation.
struct Frame {
    kind: FrameKind,
    body: String,
    nonempty: bool,
}

/// The per-container state the loop must retain between a `Start` and its
/// matching `End`.
enum FrameKind {
    /// The whole document. Its accumulated body is the walk's result.
    Document,
    Paragraph,
    Heading,
    /// Block quotes and GFM callouts share this frame; the kind arrives again
    /// on the `End` event, so only the accumulated child blocks live here.
    Quote,
    /// A fenced or indented code block. Content is buffered verbatim, because
    /// an output can only shape it once complete.
    CodeBlock {
        info: Option<String>,
        content: String,
    },
    List(ListState),
    Item,
    Table(TableState),
    TableCell,
    /// An emphasis/strong/strikethrough span. Inner content accumulates in the
    /// frame body; the closing tag hands it to the output.
    Styled(InlineStyle),
    /// A link. Its inner content accumulates in the frame body; the closing
    /// tag materializes the link per its classified kind.
    Link(LinkEmit),
    /// An image. Its alt subtree is collected as plain text (markup flattened)
    /// and handed to the output at the close.
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
    /// A raw HTML block. Its verbatim text accumulates and is handed to the
    /// output at the close.
    HtmlBlock {
        content: String,
    },
}

/// How a link materializes at its `End`, decided at `Start` from the note-link
/// table and the link's destination.
enum LinkEmit {
    /// Not a note link.
    Ordinary { dest: String },
    /// A note link whose target resolves; the inner events are dropped because
    /// the output shows the target's title.
    ResolvedNote { id: Id, title: String, slug: String },
    /// A note link whose target is dangling: the wrapper is dropped and the
    /// inner content re-emitted, so formatting in the display text survives.
    UnresolvedNote,
}

/// A list under construction. Item bodies are collected as finished strings
/// and handed over together, because tight and loose lists differ only in how
/// the items are joined.
struct ListState {
    start: Option<u64>,
    /// Detected when a paragraph opens directly inside an item.
    loose: bool,
    items: Vec<String>,
}

/// A table under construction.
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

struct Walker<'a, O: Output> {
    stack: Vec<Frame>,
    /// The note-link table, matched by byte range against Link events.
    links: &'a [ResolvedLink],
    output: &'a mut O,
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
    /// [`Walker::footnote_placeholder`] for the collision argument.
    nonce: String,
    /// Depth of open links. A positive depth suppresses autolinking, because
    /// GFM never autolinks inside a link's label.
    link_depth: usize,
    /// Depth of open images. A positive depth flattens all inner inline markup
    /// into the current image's alt text.
    image_depth: usize,
    /// The plain text of the open heading, collected beside the rendered
    /// body while a heading is open; the id and the outline entry are built
    /// from it.
    heading_text: Option<String>,
    /// Every heading id handed out so far, so a repeated one gets a suffix.
    heading_ids: Vec<String>,
}

impl<'a, O: Output> Walker<'a, O> {
    fn new(links: &'a [ResolvedLink], output: &'a mut O) -> Self {
        // GFM extended autolinks cover scheme URLs, `www.` URLs, and emails.
        // `linkify` finds scheme URLs and emails out of the box; enabling
        // `www.` needs `url_must_have_scheme(false)`, which also matches bare
        // dotted words like `report.txt`. Those false positives are filtered
        // at emission (see `text`): only real schemes, `www.` prefixes, and
        // emails become links.
        let mut finder = LinkFinder::new();
        finder.kinds(&[LinkKind::Url, LinkKind::Email]);
        finder.url_must_have_scheme(false);

        Walker {
            stack: vec![Frame {
                kind: FrameKind::Document,
                body: String::new(),
                nonempty: false,
            }],
            links,
            output,
            warnings: Vec::new(),
            finder,
            footnote_definitions: HashMap::new(),
            footnote_references: Vec::new(),
            // A fresh ULID carries 80 random bits. Wrapping the reference
            // placeholder token in this nonce makes the token unequal to any
            // substring the note can produce: the note author cannot embed a
            // value that is generated randomly, per run, after the note is
            // written. NUL is added as a second guard (no output writes it),
            // though the randomness is what makes the token safe;
            // pulldown-cmark 0.13 does not strip NUL from text, so NUL alone
            // would not suffice. Every token is substituted out before `walk`
            // returns, so the result is deterministic and nonce-free.
            nonce: format!("\u{0}ntropy-footnote-{}\u{0}", ulid::Ulid::generate()),
            link_depth: 0,
            image_depth: 0,
            heading_text: None,
            heading_ids: Vec::new(),
        }
    }

    /// Collect a heading's plain text: every text event while a heading is
    /// open, whatever inline container it sits in.
    fn push_heading_text(&mut self, text: &str) {
        if let Some(heading) = &mut self.heading_text {
            heading.push_str(text);
        }
    }

    /// The id for a heading's plain text, unique within this body.
    fn heading_id(&mut self, text: &str) -> String {
        let base = slug(text);
        let mut id = base.clone();
        let mut suffix = 0;
        while self.heading_ids.contains(&id) {
            suffix += 1;
            id = format!("{base}-{suffix}");
        }
        self.heading_ids.push(id.clone());
        id
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
        // Patch every footnote reference. A defined reference renders through
        // the output's footnote construct, once per label in the order labels
        // were first referenced, so an output that numbers footnotes numbers
        // them in reading order; an undefined one renders as its literal
        // source text (GFM). The substitutions are applied repeatedly because
        // a definition may itself contain references, whose placeholders only
        // appear once the enclosing definition is inlined; the pass count
        // bounds any self-referential cycle.
        let mut substitutions: Vec<(String, String)> = Vec::new();
        let mut seen: Vec<&String> = Vec::new();
        for label in &self.footnote_references {
            if seen.contains(&label) {
                continue;
            }
            seen.push(label);
            let placeholder = self.footnote_placeholder(label);
            let replacement = match self.footnote_definitions.get(label) {
                Some(content) => self.output.footnote(content),
                None => self.output.text(&format!("[^{label}]")),
            };
            substitutions.push((placeholder, replacement));
        }

        // The output sees the body before the placeholders are patched, so
        // whatever it appends is patched too.
        let mut body = self.output.finish(document.body);

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
            body: String::new(),
            nonempty: false,
        });
    }

    fn pop(&mut self) -> Frame {
        self.stack
            .pop()
            .expect("every end event closes a frame its start event opened")
    }

    /// Splice a finished child block into the current container. Blocks carry
    /// their own trailing newline; a single separator newline between two
    /// siblings therefore yields the blank line that separates every
    /// block-level construct.
    fn append_block(&mut self, block: &str) {
        let frame = self.top();
        if frame.nonempty {
            frame.body.push('\n');
        }
        frame.body.push_str(block);
        frame.nonempty = true;
    }

    /// Splice rendered inline content into the current container with no
    /// separation, marking it non-empty. Used by every inline construct as it
    /// closes.
    fn append_inline(&mut self, s: &str) {
        let frame = self.top();
        frame.body.push_str(s);
        frame.nonempty = true;
    }

    /// Output-owned inline content that carries no user text (the task-list
    /// box, break syntax), routed so it never lands in a code block's verbatim
    /// content or a dropped subtree.
    fn inline_syntax(&mut self, s: &str) {
        let frame = self.top();
        match &frame.kind {
            FrameKind::CodeBlock { .. } | FrameKind::HtmlBlock { .. } => {}
            _ => {
                frame.body.push_str(s);
                frame.nonempty = true;
            }
        }
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
            Event::SoftBreak => {
                self.push_heading_text(" ");
                if self.image_depth > 0 {
                    self.push_alt(" ");
                } else {
                    let s = self.output.soft_break();
                    self.inline_syntax(&s);
                }
            }
            Event::HardBreak => {
                self.push_heading_text(" ");
                if self.image_depth > 0 {
                    self.push_alt(" ");
                } else {
                    let s = self.output.hard_break();
                    self.inline_syntax(&s);
                }
            }
            Event::Rule => {
                let rule = self.output.rule();
                self.append_block(&rule);
            }
            Event::TaskListMarker(checked) => {
                let marker = self.output.task_marker(checked);
                self.inline_syntax(&marker);
            }
            Event::FootnoteReference(label) => self.footnote_reference(&label),
            // A raw HTML block's text arrives as `Html` events between its
            // `Start`/`End`; collect it for the output.
            Event::Html(html) => self.html_block_text(&html),
            Event::InlineHtml(html) => self.inline_html(&html),
        }
    }

    // -----------------------------------------------------
    // Text and inline routing
    // -----------------------------------------------------

    /// Route user text to the right place for the current container: an open
    /// image collects it as plain alt text; a code block keeps it verbatim; a
    /// raw HTML block never carries text events; everywhere else it is
    /// rendered as text, autolinked unless it sits inside a link label.
    fn text(&mut self, text: &str) {
        self.push_heading_text(text);
        if self.image_depth > 0 {
            self.push_alt(text);
            return;
        }
        let in_link = self.link_depth > 0;
        let frame = self
            .stack
            .last_mut()
            .expect("the document frame keeps the stack non-empty");
        match &mut frame.kind {
            FrameKind::CodeBlock { content, .. } => content.push_str(text),
            FrameKind::HtmlBlock { .. } => {}
            _ => {
                if in_link {
                    frame.body.push_str(&self.output.text(text));
                } else {
                    // `linkify` also surfaces scheme-less URLs, which include
                    // `www.` hosts but also bare dotted words. Only spans that
                    // carry meaning as links become links: any real scheme, a
                    // `www.` host (GFM prefixes it with `https://`), and emails
                    // (rendered as `mailto:` links). A scheme-less non-`www.`
                    // span is not a link and flows through as text.
                    for span in self.finder.spans(text) {
                        let s = span.as_str();
                        let rendered = match span.kind() {
                            Some(LinkKind::Url) if s.contains("://") => self.output.autolink(s, s),
                            Some(LinkKind::Url) if starts_with_ascii_ci(s, "www.") => {
                                self.output.autolink(&format!("https://{s}"), s)
                            }
                            Some(LinkKind::Email) => {
                                self.output.autolink(&format!("mailto:{s}"), s)
                            }
                            _ => self.output.text(s),
                        };
                        frame.body.push_str(&rendered);
                    }
                }
                frame.nonempty = true;
            }
        }
    }

    /// Inline code. Inside an image alt it degrades to the code's plain text,
    /// matching the flatten policy.
    fn code(&mut self, code: &str) {
        self.push_heading_text(code);
        if self.image_depth > 0 {
            self.push_alt(code);
            return;
        }
        let rendered = self.output.inline_code(code);
        self.append_inline(&rendered);
    }

    /// Append plain text to the innermost open image's alt buffer.
    fn push_alt(&mut self, text: &str) {
        if let FrameKind::Image { alt, .. } = &mut self.top().kind {
            alt.push_str(text);
        }
    }

    /// Record a footnote reference and drop a placeholder that `finish` patches
    /// once every definition is known. Inside an image alt a reference is
    /// meaningless and is dropped.
    fn footnote_reference(&mut self, label: &str) {
        if self.image_depth > 0 {
            return;
        }
        self.footnote_references.push(label.to_string());
        let placeholder = self.footnote_placeholder(label);
        self.append_inline(&placeholder);
    }

    /// Collect a raw HTML block's verbatim text for the output.
    fn html_block_text(&mut self, html: &str) {
        if let FrameKind::HtmlBlock { content } = &mut self.top().kind {
            content.push_str(html);
        }
    }

    /// Hand an inline raw HTML fragment to the output. Inside an image alt the
    /// fragment is dropped silently, as alt is plain text.
    fn inline_html(&mut self, html: &str) {
        if self.image_depth > 0 {
            return;
        }
        if let Some(rendered) = self.output.inline_html(html, &mut self.warnings) {
            self.append_inline(&rendered);
        }
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
            Tag::Heading { .. } => {
                self.heading_text = Some(String::new());
                self.push(FrameKind::Heading);
            }
            Tag::BlockQuote(_) => self.push(FrameKind::Quote),
            Tag::CodeBlock(kind) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => Some(info.to_string()),
                    CodeBlockKind::Indented => None,
                };
                self.push(FrameKind::CodeBlock {
                    info,
                    content: String::new(),
                });
            }
            Tag::List(start) => self.push(FrameKind::List(ListState {
                start,
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
                // the bare address as its destination; a link needs the
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
            Some(link) => match &link.target {
                Some(target) => LinkEmit::ResolvedNote {
                    id: link.id,
                    title: target.title.clone(),
                    slug: target.slug.clone(),
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
                let body = self.pop().body;
                let rendered = self.output.paragraph(&body);
                self.append_block(&rendered);
            }
            TagEnd::Heading(level) => {
                let body = self.pop().body;
                let text = self
                    .heading_text
                    .take()
                    .expect("a heading end closes the heading that set the text");
                let id = self.heading_id(&text);
                let heading = Heading { level, id, text };
                let rendered = self.output.heading(&heading, &body);
                self.append_block(&rendered);
            }
            TagEnd::BlockQuote(kind) => {
                let body = self.pop().body;
                let rendered = self.output.quote(kind, &body);
                self.append_block(&rendered);
            }
            TagEnd::CodeBlock => {
                let frame = self.pop();
                let FrameKind::CodeBlock { info, content } = frame.kind else {
                    unreachable!("a code-block end closes a code-block frame");
                };
                let rendered = self.output.code_block(info.as_deref(), &content);
                self.append_block(&rendered);
            }
            TagEnd::List(_) => {
                let frame = self.pop();
                let FrameKind::List(list) = frame.kind else {
                    unreachable!("a list end closes a list frame");
                };
                let rendered = self.output.list(&List {
                    start: list.start,
                    loose: list.loose,
                    items: list.items,
                });
                self.append_block(&rendered);
            }
            TagEnd::Item => {
                let body = self.pop().body;
                let FrameKind::List(list) = &mut self.top().kind else {
                    unreachable!("an item end exposes its enclosing list");
                };
                list.items.push(body);
            }
            TagEnd::TableCell => {
                let cell = self.pop().body;
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
                let FrameKind::Table(mut table) = frame.kind else {
                    unreachable!("a table end closes a table frame");
                };
                let columns = table.header.len();
                for row in &mut table.rows {
                    while row.len() < columns {
                        row.push(String::new());
                    }
                }
                let rendered = self.output.table(&Table {
                    alignments: table.alignments,
                    header: table.header,
                    rows: table.rows,
                });
                self.append_block(&rendered);
            }
            TagEnd::FootnoteDefinition => {
                let frame = self.pop();
                let FrameKind::FootnoteDefinition { label } = frame.kind else {
                    unreachable!("a footnote-definition end closes its frame");
                };
                // Later definitions with a repeated label overwrite earlier
                // ones, matching pulldown-cmark's own last-wins resolution.
                self.footnote_definitions.insert(label, frame.body);
            }
            TagEnd::HtmlBlock => {
                let frame = self.pop();
                let FrameKind::HtmlBlock { content } = frame.kind else {
                    unreachable!("an HTML-block end closes its frame");
                };
                if let Some(rendered) = self.output.html_block(&content, &mut self.warnings) {
                    self.append_block(&rendered);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                let frame = self.pop();
                let FrameKind::Styled(style) = frame.kind else {
                    unreachable!("an inline-style end closes a styled frame");
                };
                let rendered = self.output.styled(style, &frame.body);
                self.append_inline(&rendered);
            }
            TagEnd::Link => {
                self.link_depth -= 1;
                let frame = self.pop();
                let FrameKind::Link(emit) = frame.kind else {
                    unreachable!("a link end closes a link frame");
                };
                let inner = frame.body;
                let rendered = match emit {
                    LinkEmit::Ordinary { dest } => self.output.link(&dest, &inner),
                    LinkEmit::ResolvedNote { id, title, slug } => {
                        self.output.note_link(id, &title, &slug)
                    }
                    LinkEmit::UnresolvedNote => inner,
                };
                self.append_inline(&rendered);
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

    /// Hand the innermost image to the output and splice the result into its
    /// parent.
    fn finish_image(&mut self) {
        let frame = self.pop();
        let FrameKind::Image { dest, alt } = frame.kind else {
            unreachable!("an image end closes an image frame");
        };
        let rendered = self.output.image(&dest, &alt, &mut self.warnings);
        self.append_inline(&rendered);
    }
}

/// Resolve `dest`, a path as written in a note, against `base`, the note's
/// directory as a root-absolute path such as `/all-notes`, into a
/// root-absolute path. `.` and `..` are resolved textually; there is no
/// filesystem to consult. A `dest` that is already root-absolute passes
/// through. `None` means `dest` climbs out of the root, which no
/// root-absolute path can express.
pub fn resolve_root_relative(base: &str, dest: &str) -> Option<String> {
    if dest.starts_with('/') {
        return Some(dest.to_string());
    }
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in dest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// GitHub's heading slug: lowercase, whitespace to hyphens, keep letters,
/// digits, hyphens, and underscores, drop everything else. A heading whose
/// text leaves nothing behind gets `heading`, so every heading has an anchor.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            out.push('-');
        } else if c.is_alphanumeric() || c == '-' || c == '_' {
            out.extend(c.to_lowercase());
        }
    }
    if out.is_empty() {
        out.push_str("heading");
    }
    out
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

    use crate::render::LinkTarget;

    /// An output that renders every construct as a tagged, parenthesized
    /// form, so a test reads the walk's decisions (which construct, with
    /// which arguments, in which order) straight off the result. It escapes
    /// nothing; user text passes through as written.
    ///
    /// Two knobs simulate real outputs: an image or HTML fragment whose
    /// source contains `drop` is dropped with a warning, and with
    /// `section` set, `finish` appends the collected footnote contents the
    /// way a footnote section would.
    #[derive(Default)]
    struct Trace {
        /// Every footnote content the walk handed over, in call order.
        footnotes: Vec<String>,
        section: bool,
    }

    impl Output for Trace {
        fn text(&mut self, text: &str) -> String {
            text.to_string()
        }
        fn inline_code(&mut self, code: &str) -> String {
            format!("(code {code})")
        }
        fn soft_break(&mut self) -> String {
            "(sb)".to_string()
        }
        fn hard_break(&mut self) -> String {
            "(hb)".to_string()
        }
        fn task_marker(&mut self, checked: bool) -> String {
            format!("(task {checked})")
        }
        fn styled(&mut self, style: InlineStyle, inner: &str) -> String {
            format!("({style:?} {inner})")
        }
        fn link(&mut self, dest: &str, inner: &str) -> String {
            format!("(link {dest} {inner})")
        }
        fn note_link(&mut self, id: Id, title: &str, slug: &str) -> String {
            format!("(note {id} {slug} {title})")
        }
        fn image(&mut self, dest: &str, alt: &str, warnings: &mut Vec<Warning>) -> String {
            if dest.contains("drop") {
                warnings.push(Warning::new(format!("dropped image {dest}")));
            }
            format!("(img {dest} {alt})")
        }
        fn footnote(&mut self, content: &str) -> String {
            self.footnotes.push(content.to_string());
            format!("(fn {})", content.trim_end())
        }
        fn inline_html(&mut self, html: &str, warnings: &mut Vec<Warning>) -> Option<String> {
            if html.contains("drop") {
                warnings.push(Warning::new(format!("dropped {html}")));
                return None;
            }
            Some(format!("(ihtml {html})"))
        }
        fn paragraph(&mut self, body: &str) -> String {
            format!("(p {body})\n")
        }
        fn heading(&mut self, heading: &Heading, body: &str) -> String {
            format!(
                "({} #{} {:?} {body})\n",
                heading.level, heading.id, heading.text
            )
        }
        fn quote(&mut self, kind: Option<BlockQuoteKind>, body: &str) -> String {
            match kind {
                None => format!("(quote\n{body})\n"),
                Some(kind) => format!("(callout {kind:?}\n{body})\n"),
            }
        }
        fn code_block(&mut self, info: Option<&str>, content: &str) -> String {
            format!("(pre {info:?} {content:?})\n")
        }
        fn list(&mut self, list: &List) -> String {
            format!(
                "(list start={:?} loose={} {:?})\n",
                list.start, list.loose, list.items
            )
        }
        fn table(&mut self, table: &Table) -> String {
            format!(
                "(table {:?} {:?} {:?})\n",
                table.alignments, table.header, table.rows
            )
        }
        fn rule(&mut self) -> String {
            "(hr)\n".to_string()
        }
        fn html_block(&mut self, html: &str, warnings: &mut Vec<Warning>) -> Option<String> {
            if html.contains("drop") {
                warnings.push(Warning::new(format!("dropped {}", html.trim_end())));
                return None;
            }
            Some(format!("(html {})\n", html.trim_end()))
        }
        fn finish(&mut self, body: String) -> String {
            if self.section {
                format!("{body}\n(section {})", self.footnotes.join(" "))
            } else {
                body
            }
        }
    }

    fn trace(input: &str) -> String {
        walk(input, &[], &mut Trace::default()).0
    }

    fn note_link(range: Range<usize>, id: u64, target: Option<(&str, &str)>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: Id::from_timestamp_ms(id),
            target: target.map(|(title, slug)| LinkTarget {
                title: title.to_string(),
                slug: slug.to_string(),
            }),
        }
    }

    // =====================================================================
    // Block splicing
    // =====================================================================

    #[test]
    fn sibling_blocks_are_separated_by_one_newline() {
        // Each block carries its own trailing newline; the separator turns
        // that into a blank line between siblings and nothing after the last.
        assert_eq!(trace("a\n\nb\n\n---"), "(p a)\n\n(p b)\n\n(hr)\n");
    }

    #[test]
    fn a_dropped_html_block_leaves_no_trace_in_its_container() {
        // `None` from the output means the block never existed: no separator
        // newline is spent on it and the container stays empty until real
        // content arrives.
        let (out, warnings) = walk("<div>drop</div>\n\npara", &[], &mut Trace::default());
        assert_eq!(out, "(p para)\n");
        assert_eq!(warnings[0].message, "dropped <div>drop</div>");
    }

    #[test]
    fn a_kept_html_block_is_spliced_as_a_block() {
        assert_eq!(
            trace("<div>keep</div>\n\npara"),
            "(html <div>keep</div>)\n\n(p para)\n"
        );
    }

    #[test]
    fn inline_html_is_kept_or_dropped_per_the_output() {
        let (out, warnings) = walk(
            "a <b>keep</b> c <drop>x</drop> d",
            &[],
            &mut Trace::default(),
        );
        assert_eq!(out, "(p a (ihtml <b>)keep(ihtml </b>) c x d)\n");
        assert_eq!(warnings.len(), 2);
    }

    // =====================================================================
    // Lists and tables
    // =====================================================================

    #[test]
    fn a_paragraph_directly_inside_an_item_marks_the_list_loose() {
        assert_eq!(
            trace("- one\n\n- two"),
            "(list start=None loose=true [\"(p one)\\n\", \"(p two)\\n\"])\n"
        );
        assert_eq!(
            trace("- one\n- two"),
            "(list start=None loose=false [\"one\", \"two\"])\n"
        );
    }

    #[test]
    fn a_nested_list_does_not_make_its_parent_loose() {
        // The nested list's paragraphs open inside the nested items, two
        // frames below the outer item, so only the nested list is loose. The
        // outer item's body is its text followed by the nested list block.
        let expected = concat!(
            r#"(list start=None loose=false ["outer\n(list start=None loose=true [\"(p a)\\n\", \"(p b)\\n\"])\n"])"#,
            "\n"
        );
        assert_eq!(trace("- outer\n  - a\n\n  - b"), expected);
    }

    #[test]
    fn ordered_lists_carry_their_start_number() {
        assert_eq!(
            trace("6. six\n7. seven"),
            "(list start=Some(6) loose=false [\"six\", \"seven\"])\n"
        );
    }

    #[test]
    fn task_markers_lead_their_item() {
        assert_eq!(
            trace("- [x] done\n- [ ] open"),
            "(list start=None loose=false [\"(task true)done\", \"(task false)open\"])\n"
        );
    }

    #[test]
    fn table_rows_are_padded_to_the_header_width() {
        let out = trace("| a | b | c |\n| :-- | :-: | --: |\n| 1 |\n| 1 | 2 | 3 | 4 |");
        assert_eq!(
            out,
            "(table [Left, Center, Right] [\"a\", \"b\", \"c\"] [[\"1\", \"\", \"\"], [\"1\", \"2\", \"3\"]])\n"
        );
    }

    // =====================================================================
    // Links
    // =====================================================================

    #[test]
    fn link_events_are_classified_by_their_byte_span() {
        let input = "[t](01ARZ3NDEKTSV4RRFFQ69G5FAV-x.md) [u](01ARZ3NDEKTSV4RRFFQ69G5FAV-y.md) [o](https://e.org)";
        let resolved = 0..input.find(" [u]").expect("second link");
        let dangling = resolved.end + 1..input.find(" [o]").expect("third link");
        let links = [
            note_link(resolved, 0, Some(("Title", "slug"))),
            note_link(dangling, 0, None),
        ];
        let (out, _) = walk(input, &links, &mut Trace::default());
        assert_eq!(
            out,
            "(p (note 00000000000000000000000000 slug Title) u (link https://e.org o))\n"
        );
    }

    #[test]
    fn an_unresolved_note_link_keeps_its_inner_markup() {
        let input = "[**bold** ghost](01ARZ3NDEKTSV4RRFFQ69G5FAV-x.md)";
        let links = [note_link(0..input.len(), 0, None)];
        let (out, _) = walk(input, &links, &mut Trace::default());
        assert_eq!(out, "(p (Strong bold) ghost)\n");
    }

    #[test]
    fn autolinks_are_detected_per_gfm_and_skipped_inside_link_labels() {
        assert_eq!(
            trace("see https://a.io and www.b.org or me@c.dev not report.txt"),
            "(p see (link https://a.io https://a.io) and (link https://www.b.org www.b.org) or (link mailto:me@c.dev me@c.dev) not report.txt)\n"
        );
        assert_eq!(
            trace("[label www.b.org](https://e.org)"),
            "(p (link https://e.org label www.b.org))\n"
        );
    }

    #[test]
    fn an_angle_bracket_email_gains_the_mailto_scheme() {
        assert_eq!(trace("<me@c.dev>"), "(p (link mailto:me@c.dev me@c.dev))\n");
    }

    // =====================================================================
    // Images
    // =====================================================================

    #[test]
    fn an_image_alt_flattens_every_nested_construct_to_text() {
        assert_eq!(
            trace("![a *b* `c` [d](e) ![f](g)\nh](img.png)"),
            "(p (img img.png a b c d f h))\n"
        );
    }

    #[test]
    fn an_image_the_output_cannot_carry_still_warns_through_the_walk() {
        let (out, warnings) = walk("![alt](drop.png)", &[], &mut Trace::default());
        assert_eq!(out, "(p (img drop.png alt))\n");
        assert_eq!(warnings[0].message, "dropped image drop.png");
    }

    // =====================================================================
    // Footnotes
    // =====================================================================

    #[test]
    fn the_footnote_construct_runs_once_per_label_in_first_reference_order() {
        let mut output = Trace::default();
        let (out, _) = walk("a[^y] b[^x] c[^y]\n\n[^x]: X\n\n[^y]: Y", &[], &mut output);
        assert_eq!(output.footnotes, vec!["(p Y)\n", "(p X)\n"]);
        assert_eq!(out, "(p a(fn (p Y)) b(fn (p X)) c(fn (p Y)))\n");
    }

    #[test]
    fn an_undefined_reference_renders_as_its_source_text() {
        assert_eq!(trace("a[^ghost]"), "(p a[^ghost])\n");
    }

    #[test]
    fn an_unreferenced_definition_produces_nothing() {
        assert_eq!(trace("a\n\n[^x]: unused"), "(p a)\n");
    }

    #[test]
    fn a_definition_referencing_another_footnote_is_patched_recursively() {
        // The nested reference's placeholder only enters the body once the
        // outer definition is inlined; the repeated passes patch it too.
        let (out, _) = walk(
            "x[^a]\n\n[^a]: sees[^b]\n\n[^b]: B",
            &[],
            &mut Trace::default(),
        );
        assert!(!out.contains('\u{0}'), "placeholder leaked: {out:?}");
        assert_eq!(out, "(p x(fn (p sees(fn (p B)))))\n");
    }

    #[test]
    fn a_self_referencing_definition_terminates() {
        let (out, _) = walk("x[^a]\n\n[^a]: me[^a]", &[], &mut Trace::default());
        // The pass count bounds the expansion; what remains after the last
        // pass is one unpatched placeholder, which is the accepted outcome
        // for a cycle no output could render anyway.
        assert!(out.starts_with("(p x(fn (p me(fn (p me"));
    }

    #[test]
    fn finish_runs_before_placeholders_are_patched() {
        // The section `finish` appends is built from stored definition
        // contents, which still hold the raw placeholder of the nested
        // reference; it is patched only because `finish` runs first.
        let mut output = Trace {
            footnotes: Vec::new(),
            section: true,
        };
        let (out, _) = walk("x[^a]\n\n[^a]: sees[^b]\n\n[^b]: B", &[], &mut output);
        assert!(
            out.ends_with("\n(section (p sees(fn (p B)))\n (p B)\n)"),
            "finish output missing or unpatched: {out:?}"
        );
        assert!(!out.contains('\u{0}'), "placeholder leaked: {out:?}");
    }

    // =====================================================================
    // Inline routing
    // =====================================================================

    #[test]
    fn breaks_and_inline_code_route_through_the_output() {
        assert_eq!(trace("a\nb  \nc `d`"), "(p a(sb)b(hb)c (code d))\n");
    }

    #[test]
    fn text_inside_a_code_block_is_kept_verbatim_with_its_info_string() {
        assert_eq!(
            trace("```rust ignore\nfn x() {}\n```"),
            "(pre Some(\"rust ignore\") \"fn x() {}\\n\")\n"
        );
        // An indented block's final line carries no newline of its own.
        assert_eq!(trace("    indented"), "(pre None \"indented\")\n");
    }

    #[test]
    fn callouts_carry_their_kind_and_quotes_none() {
        assert_eq!(
            trace("> [!TIP]\n> t\n\n> q"),
            "(callout Tip\n(p t)\n)\n\n(quote\n(p q)\n)\n"
        );
    }

    // =====================================================================
    // Headings
    // =====================================================================

    #[test]
    fn a_heading_carries_its_level_id_and_flattened_text() {
        assert_eq!(
            trace("## Hello *big* `wide` [world](https://e.org)!"),
            "(h2 #hello-big-wide-world \"Hello big wide world!\" Hello (Emph big) (code wide) (link https://e.org world)!)\n"
        );
    }

    #[test]
    fn heading_ids_follow_githubs_rule() {
        assert_eq!(slug("Hello World"), "hello-world");
        assert_eq!(slug("  Two   spaces "), "--two---spaces-");
        assert_eq!(slug("Keep_under-scores"), "keep_under-scores");
        assert_eq!(
            slug("Drop: punctuation, (parens) & symbols!"),
            "drop-punctuation-parens--symbols"
        );
        assert_eq!(slug("Über Größe 日本語"), "über-größe-日本語");
        assert_eq!(slug("1.2 Numbers"), "12-numbers");
        assert_eq!(slug("!!!"), "heading");
        assert_eq!(slug(""), "heading");
    }

    #[test]
    fn repeated_heading_ids_get_numbered_suffixes() {
        assert_eq!(
            trace("# A\n\n# A\n\n# A-1\n\n# A"),
            "(h1 #a \"A\" A)\n\n(h1 #a-1 \"A\" A)\n\n(h1 #a-1-1 \"A-1\" A-1)\n\n(h1 #a-2 \"A\" A)\n"
        );
    }

    #[test]
    fn a_setext_heading_joins_its_lines_with_a_space() {
        assert_eq!(
            trace("Line one\nline two\n========"),
            "(h1 #line-one-line-two \"Line one line two\" Line one(sb)line two)\n"
        );
    }

    #[test]
    fn an_image_inside_a_heading_contributes_its_alt_to_the_text() {
        assert_eq!(
            trace("# See ![the *plan*](p.png) now"),
            "(h1 #see-the-plan-now \"See the plan now\" See (img p.png the plan) now)\n"
        );
    }

    #[test]
    fn root_relative_resolution_joins_climbs_and_refuses_escapes() {
        assert_eq!(
            resolve_root_relative("/all-notes", "diagram.png").as_deref(),
            Some("/all-notes/diagram.png")
        );
        assert_eq!(
            resolve_root_relative("/all-notes", "./pics/x.png").as_deref(),
            Some("/all-notes/pics/x.png")
        );
        assert_eq!(
            resolve_root_relative("/all-notes", "../assets/logo.svg").as_deref(),
            Some("/assets/logo.svg")
        );
        assert_eq!(
            resolve_root_relative("/all-notes", "/assets/logo.svg").as_deref(),
            Some("/assets/logo.svg")
        );
        assert_eq!(
            resolve_root_relative("/all-notes", "../../outside.png"),
            None
        );
        assert_eq!(
            resolve_root_relative("/", "x.png").as_deref(),
            Some("/x.png")
        );
    }

    #[test]
    fn www_prefix_test_is_byte_safe_across_multibyte_boundaries() {
        // A short multi-byte string must not panic the prefix test, which
        // guards the `www.` autolink rule against `str` slicing.
        assert!(!starts_with_ascii_ci("wwü", "www."));
        assert!(starts_with_ascii_ci("WWW.example.com", "www."));
    }
}
