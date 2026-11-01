// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The ansi engine: a note drawn for the terminal by patine (ADR 0082,
//! `docs/design/ansi-engine.md`).
//!
//! patine lays the note out as styled lines; ntropy writes those lines
//! itself, so that every piece of text passes through the escaping of
//! ADR 0071 on its way out and the drawing can leave out the colours or every
//! style.

use std::fmt::Write as _;

use patine::{QuoteStyle, StyledSpan, TextStyle, Underline};

use super::{PreparedDocument, RenderContext, RenderError, Renderer};

/// The width the `ansi` artifact is laid out at, and the width `show` uses
/// when no terminal tells it one.
pub const DEFAULT_WIDTH: usize = 80;

/// Which escape sequences a drawing carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// The text alone, for output that is not a terminal.
    Plain,
    /// Every style but the colours, for a terminal under `NO_COLOR`.
    Styled,
    /// Every style patine gives, colours included.
    Colored,
}

/// How the engine lays a note out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnsiOptions {
    /// The columns patine wraps running text at.
    pub width: usize,
    pub form: Form,
}

/// A drawn note: the text to write, and the display width of each of its
/// lines, so a caller can tell how many terminal rows it takes.
#[derive(Debug, Clone)]
pub struct Drawn {
    pub text: String,
    line_widths: Vec<usize>,
}

impl Drawn {
    /// The terminal rows the drawing takes at `columns`, each line counted as
    /// the rows it wraps into. An empty line still takes a row.
    pub fn rows(&self, columns: usize) -> usize {
        let columns = columns.max(1);
        self.line_widths
            .iter()
            .map(|width| width.div_ceil(columns).max(1))
            .sum()
    }
}

/// Draw a note from its frontmatter block, as written between its fences,
/// and its body.
pub fn draw_source(frontmatter: &str, body: &str, options: &AnsiOptions) -> Drawn {
    // The fences are written as plain `---`. ntropy accepts a fence followed
    // by any whitespace, patine only by spaces and tabs, and patine draws the
    // fences from the marker either way; the block and the body go over as
    // the note has them.
    let source = format!("---\n{frontmatter}---\n{body}");

    // Without colours the quote bars would be cyan; dim bars keep them apart
    // from the quoted text.
    let mut patine_options = patine::RenderOptions::new();
    if options.form == Form::Styled {
        patine_options = patine_options.with_quote_style(QuoteStyle::DimBar);
    }
    let lines = patine::render_lines_with(&source, options.width, &patine_options);

    let mut text = String::new();
    let mut line_widths = Vec::with_capacity(lines.len());
    for line in &lines {
        let mut column = 0;
        for span in &line.spans {
            let shown = shown_text(&span.text, column);
            column += text_width(&shown);
            write_span(&mut text, &shown, span.style, options.form);
        }
        text.push('\n');
        line_widths.push(column);
    }
    Drawn { text, line_widths }
}

/// [`draw_source`] over a prepared document.
pub fn draw(doc: &PreparedDocument, options: &AnsiOptions) -> Drawn {
    draw_source(&doc.frontmatter_source, &doc.body, options)
}

/// The columns between two tab stops, the terminals' default.
const TAB_STOP: usize = 8;

/// A span's text as it is written, for a span starting at `column`.
///
/// patine writes text verbatim, and a numeric character reference such as
/// `&#x1b;` decodes to the character it names, so this is the one place every
/// control character can be caught (ADR 0071). A tab, which code blocks keep,
/// becomes the spaces to the next tab stop counted from the start of the
/// line: the drawing then moves no cursor and its width is known.
fn shown_text(text: &str, column: usize) -> String {
    let mut shown = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\t' {
            let at = column + text_width(&shown);
            let spaces = TAB_STOP - at % TAB_STOP;
            shown.extend(std::iter::repeat_n(' ', spaces));
        } else if crate::text::escape::needs_escape(ch) {
            shown.extend(ch.escape_debug());
        } else {
            shown.push(ch);
        }
    }
    shown
}

/// The columns `text` takes, counted as patine counts them when it lays a
/// line out.
fn text_width(text: &str) -> usize {
    StyledSpan::new(text, TextStyle::default()).width()
}

/// Write one span in `form`: the escape sequences that switch its attributes
/// on, its text, and the ones that switch them off again in reverse order.
///
/// The sequences are the ones patine's own writer uses, so a span starts
/// and ends in the terminal's default style and nothing carries over a line
/// break. Bold and dim share one reset, SGR 22.
fn write_span(out: &mut String, text: &str, style: TextStyle, form: Form) {
    if form == Form::Plain {
        out.push_str(text);
        return;
    }
    let foreground = match form {
        Form::Colored => style.foreground,
        Form::Styled | Form::Plain => None,
    };
    if style.bold {
        out.push_str("\x1b[1m");
    }
    if style.dim {
        out.push_str("\x1b[2m");
    }
    if style.italic {
        out.push_str("\x1b[3m");
    }
    match style.underline {
        Underline::None => {}
        Underline::Single => out.push_str("\x1b[4m"),
        Underline::Double => out.push_str("\x1b[4:2m"),
    }
    if style.strikethrough {
        out.push_str("\x1b[9m");
    }
    if let Some(color) = foreground {
        let _ = write!(out, "\x1b[38;5;{}m", color.index());
    }
    out.push_str(text);
    if foreground.is_some() {
        out.push_str("\x1b[39m");
    }
    if style.strikethrough {
        out.push_str("\x1b[29m");
    }
    if style.underline != Underline::None {
        out.push_str("\x1b[24m");
    }
    if style.italic {
        out.push_str("\x1b[23m");
    }
    if style.bold || style.dim {
        out.push_str("\x1b[22m");
    }
}

/// The engine registered as `ansi`: the drawing at fixed options is the
/// artifact.
pub struct Ansi {
    options: AnsiOptions,
}

impl Ansi {
    pub fn new(options: AnsiOptions) -> Self {
        Ansi { options }
    }
}

impl Renderer for Ansi {
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError> {
        ctx.write_output(draw(doc, &self.options).text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::render::{Invocation, ToolOutput};

    const KITCHEN_SINK: &str = include_str!("../../tests/fixtures/kitchen-sink.md");

    fn options(width: usize, form: Form) -> AnsiOptions {
        AnsiOptions { width, form }
    }

    /// The note's file split the way ntropy splits it, drawn.
    fn draw_note(file: &str, width: usize, form: Form) -> Drawn {
        let split = crate::note::frontmatter::split(file);
        draw_source(
            split
                .frontmatter
                .expect("the fixture has a frontmatter block"),
            split.body,
            &options(width, form),
        )
    }

    /// patine's own lines for `source`, as plain text.
    fn patine_plain(source: &str, width: usize) -> String {
        patine::render_lines_with(source, width, &patine::RenderOptions::new())
            .iter()
            .map(|line| format!("{line}\n"))
            .collect()
    }

    /// `text` with every well-formed SGR sequence taken out.
    fn without_sgr(text: &str) -> String {
        regex::Regex::new(r"\x1b\[[0-9;:]*m")
            .expect("the pattern is valid")
            .replace_all(text, "")
            .into_owned()
    }

    /// The display width of each line of `text`, SGR sequences removed.
    fn line_widths(text: &str) -> Vec<usize> {
        without_sgr(text)
            .lines()
            .map(unicode_width::UnicodeWidthStr::width)
            .collect()
    }

    // =========================================================
    // What patine is handed
    // =========================================================

    #[test]
    fn a_plain_drawing_is_patines_layout_of_the_note() {
        let note = "---\ntitle: Plan\ntags: [work]\n---\n# Plan\n\nSome *text* here.\n";
        assert_eq!(
            draw_note(note, 80, Form::Plain).text,
            patine_plain(note, 80)
        );
    }

    #[test]
    fn the_frontmatter_is_drawn_as_patines_frontmatter_block() {
        let note = "---\ntitle: T\ntags: [a]\n---\n# T\n\nBody.\n";
        let drawn = draw_note(note, 80, Form::Plain).text;
        let lines: Vec<&str> = drawn.lines().map(str::trim).collect();
        assert_eq!(
            lines[..4],
            ["---", "title: T", "tags: [a]", "---"],
            "{drawn}"
        );
        // The heading is the note's own, drawn after the block.
        assert!(lines[4..].contains(&"T"), "{drawn}");
        assert!(lines[4..].contains(&"Body."), "{drawn}");
    }

    #[test]
    fn a_fence_ntropy_accepts_with_trailing_unicode_space_is_still_a_fence() {
        // ntropy trims any whitespace after `---`, patine only spaces and
        // tabs. Handed the file as it is, patine would read the YAML as a
        // setext heading.
        let file = "---\u{a0}\ntitle: T\n---\nBody.\n";
        let note = crate::note::Note::parse(
            PathBuf::from("/vault/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-t.md"),
            file,
            None,
        )
        .expect("ntropy accepts the fence");
        let index = crate::link::index(std::slice::from_ref(&note));
        let doc = crate::render::prepare(&note, &index, Path::new("/vault")).expect("prepare");
        let drawn = draw(&doc, &options(80, Form::Plain)).text;
        let lines: Vec<&str> = drawn.lines().map(str::trim).collect();
        assert_eq!(lines[..3], ["---", "title: T", "---"], "{drawn}");
    }

    #[test]
    fn draw_is_draw_source_over_the_documents_fields() {
        let split = crate::note::frontmatter::split(KITCHEN_SINK);
        let note = crate::note::Note::parse(
            PathBuf::from("/vault/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-kitchen-sink.md"),
            KITCHEN_SINK,
            None,
        )
        .expect("the fixture parses");
        let index = crate::link::index(std::slice::from_ref(&note));
        let doc = crate::render::prepare(&note, &index, Path::new("/vault")).expect("prepare");
        let options = options(80, Form::Colored);
        let drawn = draw(&doc, &options).text;
        assert!(drawn.contains("Kitchen Sink"), "{drawn:?}");
        assert_eq!(
            drawn,
            draw_source(
                split.frontmatter.expect("the fixture has a block"),
                split.body,
                &options
            )
            .text
        );
    }

    // =========================================================
    // The escape sequences
    // =========================================================

    #[test]
    fn a_colored_drawing_matches_patines_own_writer() {
        let styles = "---\ntitle: S\n---\n# One\n\n## Two\n\n*it* **bo** ~~st~~ `co` \
                      [li](https://example.org)\n\n- [x] **x**\n\n> quoted\n\n\
                      ```rust\nfn main() {}\n```\n";
        for note in [KITCHEN_SINK, styles] {
            let mut expected = Vec::new();
            patine::render(note, &mut expected, 80).expect("writing to a Vec cannot fail");
            assert_eq!(
                draw_note(note, 80, Form::Colored).text,
                String::from_utf8(expected).expect("patine writes UTF-8")
            );
        }
    }

    #[test]
    fn a_styled_drawing_is_the_colored_one_without_colours_and_with_dim_bars() {
        let note = "---\ntitle: S\n---\n# One\n\n`code` and > text\n\n> quoted\n";
        let styled = draw_note(note, 80, Form::Styled).text;
        assert!(!styled.contains("\x1b[38;5;"), "{styled:?}");
        assert!(!styled.contains("\x1b[39m"), "{styled:?}");
        assert!(styled.contains("\x1b[4:2m"), "{styled:?}");
        // The quote bar is dim, as patine's `DimBar` draws it.
        assert!(styled.contains("\x1b[2m│ \x1b[22m"), "{styled:?}");
        assert_eq!(
            without_sgr(&styled),
            without_sgr(&draw_note(note, 80, Form::Colored).text)
        );
    }

    #[test]
    fn a_plain_drawing_holds_no_escape_sequence() {
        let drawn = draw_note(KITCHEN_SINK, 80, Form::Plain).text;
        assert!(!drawn.is_empty());
        assert!(!drawn.contains('\x1b'), "{drawn:?}");
    }

    #[test]
    fn a_note_link_stays_a_link_with_its_target_dimmed() {
        let note = "---\ntitle: L\n---\nSee [Plan](01BX5ZZKBKACTAV9WEVGEMMVRZ-plan.md).\n";
        let drawn = draw_note(note, 80, Form::Colored).text;
        assert!(
            drawn.contains(
                "\x1b[4mPlan\x1b[24m\x1b[2m (01BX5ZZKBKACTAV9WEVGEMMVRZ-plan.md)\x1b[22m"
            ),
            "{drawn:?}"
        );
    }

    // =========================================================
    // Text from the vault
    // =========================================================

    #[test]
    fn control_and_bidi_characters_are_escaped_wherever_they_come_from() {
        let hostile = "---\ntitle: H\n---\nraw \x1b[2J and \u{9b}6n and \u{202e}txt, \
                       decoded &#x1b;[31m and &#27;]52;c;x and \
                       [a](x&#13;y) [b](x&#10;y) [c](x&#x1b;y)\n";
        let clean = "---\ntitle: H\n---\nraw [2J and 6n and txt, \
                     decoded [31m and ]52;c;x and \
                     [a](xy) [b](xy) [c](xy)\n";
        let drawn = draw_note(hostile, 200, Form::Colored).text;
        for escaped in [
            "\\u{1b}[2J",
            "\\u{9b}6n",
            "\\u{202e}txt",
            "\\u{1b}[31m",
            "\\u{1b}]52;c;x",
            "(x\\ry)",
            "(x\\ny)",
            "(x\\u{1b}y)",
        ] {
            assert!(drawn.contains(escaped), "{escaped} in {drawn:?}");
        }
        assert!(!without_sgr(&drawn).contains('\x1b'), "{drawn:?}");
        assert_eq!(
            drawn.lines().count(),
            draw_note(clean, 200, Form::Colored).text.lines().count()
        );
    }

    #[test]
    fn a_control_character_in_a_frontmatter_value_is_escaped() {
        // patine shows the block as written, so only a raw control character
        // in the file reaches the drawing, and it is escaped there.
        let note = "---\ntitle: T\x1b[2J\n---\nBody.\n";
        let drawn = draw_note(note, 80, Form::Plain).text;
        assert!(drawn.contains("T\\u{1b}[2J"), "{drawn:?}");
        assert!(!drawn.contains('\x1b'), "{drawn:?}");
    }

    #[test]
    fn a_tab_becomes_the_spaces_to_the_next_tab_stop() {
        let note = "---\ntitle: T\n---\n```\na\tb\n\tc\n```\n";
        let drawn = draw_note(note, 80, Form::Plain);
        assert!(!drawn.text.contains('\t'), "{:?}", drawn.text);
        // The code block's indent is two columns, so `a` sits in column 2
        // and the tab runs to column 8; a leading tab runs from 2 to 8.
        assert!(drawn.text.contains("\n  a     b\n"), "{:?}", drawn.text);
        assert!(drawn.text.contains("\n        c\n"), "{:?}", drawn.text);
        assert_eq!(drawn.line_widths, line_widths(&drawn.text));
    }

    // =========================================================
    // Width and rows
    // =========================================================

    #[test]
    fn running_text_wraps_at_the_width() {
        let words = "word ".repeat(40);
        let note = format!("---\ntitle: W\n---\n{words}\n");
        for width in [30, 80] {
            let drawn = draw_note(&note, width, Form::Plain);
            assert!(drawn.text.lines().count() > 3, "{:?}", drawn.text);
            assert!(
                line_widths(&drawn.text).iter().all(|&w| w <= width),
                "{width}: {:?}",
                drawn.text
            );
        }
    }

    #[test]
    fn rows_count_each_line_as_the_rows_it_wraps_into() {
        let drawn = Drawn {
            text: String::new(),
            line_widths: vec![10, 0, 95],
        };
        assert_eq!(drawn.rows(40), 5);
    }

    #[test]
    fn rows_count_a_tab_by_the_columns_it_takes() {
        // Ten tabs and twenty characters are 100 columns wide once the tabs
        // reach their stops, two rows at 80 columns.
        let line = format!("{}{}", "\t".repeat(10), "x".repeat(20));
        let note = format!("---\ntitle: T\n---\n```\n{line}\n```\n");
        let drawn = draw_note(&note, 80, Form::Plain);
        let code_line = drawn
            .text
            .lines()
            .position(|l| l.ends_with('x'))
            .expect("the code line is drawn");
        assert_eq!(drawn.line_widths[code_line], 100);
        let others = drawn.line_widths.len() - 1;
        assert_eq!(drawn.rows(80), others + 2);
    }

    #[test]
    fn rows_count_a_halfwidth_sound_mark_as_patine_lays_it_out() {
        // patine gives `ﾞ` a column of its own, which a whole-string
        // Unicode width does not.
        let drawn = draw_note("---\ntitle: K\n---\nｶﾞ\n", 80, Form::Plain);
        let line = drawn
            .text
            .lines()
            .position(|l| l.contains('ﾞ'))
            .expect("the line is drawn");
        assert_eq!(drawn.line_widths[line], 2);
    }

    // =========================================================
    // The engine
    // =========================================================

    /// A context that records the artifact and any warning.
    struct Recording {
        written: Vec<Vec<u8>>,
        warnings: Vec<String>,
    }

    impl RenderContext for Recording {
        fn stage_file(&mut self, _name: &str, _contents: &[u8]) -> Result<PathBuf, RenderError> {
            unreachable!("the ansi engine stages nothing")
        }

        fn run(&mut self, _invocation: &Invocation) -> Result<ToolOutput, RenderError> {
            unreachable!("the ansi engine runs no tool")
        }

        fn write_output(&mut self, contents: &[u8]) -> Result<(), RenderError> {
            self.written.push(contents.to_vec());
            Ok(())
        }

        fn write_file(&mut self, _relative: &str, _contents: &[u8]) -> Result<(), RenderError> {
            unreachable!("the ansi engine writes no files beside the artifact")
        }

        fn copy_file(&mut self, _from: &Path, _relative: &str) -> Result<(), RenderError> {
            unreachable!("the ansi engine copies no files")
        }

        fn warn(&mut self, message: &str) {
            self.warnings.push(message.to_string());
        }

        fn output_path(&self) -> &Path {
            Path::new("/artifacts/note.ansi")
        }
    }

    #[test]
    fn the_engine_writes_the_colored_drawing_as_the_artifact() {
        let note = crate::note::Note::parse(
            PathBuf::from("/vault/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-kitchen-sink.md"),
            KITCHEN_SINK,
            None,
        )
        .expect("the fixture parses");
        let index = crate::link::index(std::slice::from_ref(&note));
        let doc = crate::render::prepare(&note, &index, Path::new("/vault")).expect("prepare");
        let options = options(DEFAULT_WIDTH, Form::Colored);
        let mut ctx = Recording {
            written: Vec::new(),
            warnings: Vec::new(),
        };
        Ansi::new(options)
            .render(&doc, &mut ctx)
            .expect("the render succeeds");
        assert_eq!(ctx.written, [draw(&doc, &options).text.into_bytes()]);
        assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
    }
}
