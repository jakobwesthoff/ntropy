// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The ansi engine: a note drawn for the terminal by patine (ADR 0082,
//! `docs/design/ansi-engine.md`).
//!
//! patine lays the note out as styled lines and writes them. In between,
//! ntropy escapes every span's text (ADR 0071) and restyles the spans, so
//! the drawing can leave out the colours or every style.

use patine::{CalloutStyle, QuoteStyle, StyledLine, StyledSpan, TextStyle};

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

    // Without colours the quote bars would be cyan and the callout bars
    // heavy and undimmed; dim bars keep them apart from the text. patine
    // styles callouts independently of quotes, so both are set.
    let mut patine_options = patine::RenderOptions::new();
    if options.form == Form::Styled {
        patine_options = patine_options
            .with_quote_style(QuoteStyle::DimBar)
            .with_callout_style(CalloutStyle::DimBar);
    }
    let lines = patine::render_lines_with(&source, options.width, &patine_options);

    let mut shown_lines = Vec::with_capacity(lines.len());
    let mut line_widths = Vec::with_capacity(lines.len());
    for line in &lines {
        let mut column = 0;
        let mut spans = Vec::with_capacity(line.spans.len());
        for span in &line.spans {
            let shown = shown_text(&span.text, column);
            column += text_width(&shown);
            spans.push(StyledSpan::new(shown, form_style(span.style, options.form)));
        }
        shown_lines.push(StyledLine::new(spans));
        line_widths.push(column);
    }
    let text = patine::ansi(&shown_lines).to_string();
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
    // The column where the text since the last tab starts, and where that
    // text starts in `shown`. Each run between two tabs is measured once, so
    // the cost stays in proportion to the text; a grapheme never spans a
    // tab, so measuring by runs counts as patine counts the whole line.
    let mut run_column = column;
    let mut run_start = 0;
    for ch in text.chars() {
        if ch == '\t' {
            let at = run_column + text_width(&shown[run_start..]);
            let spaces = TAB_STOP - at % TAB_STOP;
            shown.extend(std::iter::repeat_n(' ', spaces));
            run_column = at + spaces;
            run_start = shown.len();
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

/// The style a span is written in under `form`.
///
/// `patine::ansi` writes no sequence for the default style, so `Plain` is
/// the text alone. `without_colors` is patine's own decision of what counts
/// as a colour, which keeps a style patine adds later in the drawing.
fn form_style(style: TextStyle, form: Form) -> TextStyle {
    match form {
        Form::Colored => style,
        Form::Styled => style.without_colors(),
        Form::Plain => TextStyle::default(),
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
    // Footnotes
    // =========================================================

    /// The plain drawing of a body under a minimal frontmatter block.
    fn plain_body(body: &str) -> String {
        draw_source("title: T\n", body, &options(80, Form::Plain)).text
    }

    #[test]
    fn a_footnote_only_an_unreferenced_footnote_mentions_is_not_drawn() {
        // `[^b]` is mentioned by `[^z]` alone and nothing refers to `[^z]`.
        // patine 2.2.1 panicked on this shape.
        let drawn = plain_body("text\n\n[^z]: sees[^b]\n\n[^b]: B\n\nlater[^c]\n\n[^c]: C\n");
        assert!(drawn.contains("later[1]"), "{drawn}");
        assert!(drawn.contains("[1] C"), "{drawn}");
        assert!(!drawn.contains("sees"), "{drawn}");
        assert!(!drawn.contains("[1] B"), "{drawn}");
    }

    #[test]
    fn footnotes_that_refer_only_to_each_other_are_not_drawn() {
        let drawn = plain_body("text\n\n[^a]: [^b]\n\n[^b]: [^a]\n");
        assert!(!drawn.contains("[1]"), "{drawn}");
    }

    #[test]
    fn a_reference_in_image_alt_text_draws_no_footnote() {
        let drawn = plain_body("![a[^1]](u)\n\n[^1]: n\n");
        assert!(drawn.contains("[image: a]"), "{drawn}");
        assert!(!drawn.contains("[1]"), "{drawn}");
    }

    // =========================================================
    // The escape sequences
    // =========================================================

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
        for form in [Form::Plain, Form::Styled, Form::Colored] {
            let drawn = draw_note(hostile, 200, form).text;
            for escaped in [
                "\\u{1b}[2J",
                "\\u{9b}6n",
                "\\u{202e}txt",
                "\\u{1b}[31m",
                "\\u{1b}]52;c;x",
                "(x%0Dy)",
                "(x%0Ay)",
                "(x\\u{1b}y)",
            ] {
                assert!(drawn.contains(escaped), "{form:?}: {escaped} in {drawn:?}");
            }
            assert!(!without_sgr(&drawn).contains('\x1b'), "{form:?}: {drawn:?}");
            assert_eq!(
                drawn.lines().count(),
                draw_note(clean, 200, form).text.lines().count()
            );
        }
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

    // =========================================================
    // Link targets with a line ending
    // =========================================================

    /// Assert that `body`, drawn in every form with its SGR sequences
    /// removed, holds a line that is `expected` once its indent is trimmed.
    fn assert_draws_line(body: &str, expected: &str) {
        for form in [Form::Plain, Form::Styled, Form::Colored] {
            let drawn = draw_source("title: T\n", body, &options(200, form)).text;
            let plain = without_sgr(&drawn);
            assert!(
                plain.lines().any(|line| line.trim() == expected),
                "{form:?}: {expected:?} in {plain:?}"
            );
        }
    }

    // A character reference decodes to the line ending it names. patine
    // writes it into the target's suffix as `%0D` or `%0A`, and a link whose
    // text repeats such a target still gets the suffix.

    #[test]
    fn a_carriage_return_in_a_link_target_is_drawn_as_percent_0d() {
        assert_draws_line("[a](x&#13;y)\n", "a (x%0Dy)");
    }

    #[test]
    fn a_line_feed_in_a_link_target_is_drawn_as_percent_0a() {
        assert_draws_line("[a](x&#10;y)\n", "a (x%0Ay)");
    }

    #[test]
    fn a_carriage_return_in_an_image_source_is_drawn_as_percent_0d() {
        assert_draws_line("![img](x&#13;y)\n", "[image: img] (x%0Dy)");
    }

    #[test]
    fn a_link_whose_text_repeats_a_target_with_a_line_ending_shows_the_target() {
        assert_draws_line("[a&#10;b](a&#10;b)\n", "a b (a%0Ab)");
    }

    #[test]
    fn an_autolink_with_a_line_ending_shows_the_target() {
        assert_draws_line("<http://x/a&#10;b>\n", "http://x/a b (http://x/a%0Ab)");
    }

    // The target is only added for a line ending: a link that repeats its
    // target and an autolink without one stay bare.

    #[test]
    fn a_link_whose_text_repeats_its_target_shows_no_suffix() {
        assert_draws_line(
            "[https://example.com](https://example.com)\n",
            "https://example.com",
        );
    }

    #[test]
    fn an_autolink_without_a_line_ending_shows_no_suffix() {
        assert_draws_line("<https://example.com>\n", "https://example.com");
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
    // Callouts
    // =========================================================

    /// A note holding one alert of `marker`'s kind with a one-line body.
    fn alert_note(marker: &str) -> String {
        format!("---\ntitle: A\n---\n> [!{marker}]\n> Body.\n")
    }

    #[test]
    fn a_colored_drawing_paints_a_callout_in_the_kinds_colour() {
        for (marker, color, symbol, title) in [
            ("NOTE", patine::AnsiColor::Blue, '●', "Note"),
            ("TIP", patine::AnsiColor::Green, '◆', "Tip"),
            ("IMPORTANT", patine::AnsiColor::Magenta, '■', "Important"),
            ("WARNING", patine::AnsiColor::Yellow, '▲', "Warning"),
            ("CAUTION", patine::AnsiColor::Red, '✕', "Caution"),
        ] {
            let drawn = draw_note(&alert_note(marker), 80, Form::Colored).text;
            let index = color.index();
            let bar = format!("\x1b[38;5;{index}m┃ \x1b[39m");
            let heading = format!("\x1b[1m\x1b[38;5;{index}m{symbol} {title}\x1b[39m\x1b[22m");
            assert!(drawn.contains(&bar), "{marker}: {bar:?} in {drawn:?}");
            assert!(
                drawn.contains(&heading),
                "{marker}: {heading:?} in {drawn:?}"
            );
        }
    }

    #[test]
    fn a_styled_drawing_dims_the_bar_of_a_callout_and_keeps_its_title_bold() {
        let drawn = draw_note(&alert_note("WARNING"), 80, Form::Styled).text;
        assert!(drawn.contains("\x1b[2m┃ \x1b[22m"), "{drawn:?}");
        assert!(drawn.contains("\x1b[1m▲ Warning\x1b[22m"), "{drawn:?}");
        assert!(!drawn.contains("\x1b[38;5;"), "{drawn:?}");
    }

    #[test]
    fn a_plain_drawing_shows_a_callout_as_text_alone() {
        let drawn = draw_note(&alert_note("WARNING"), 80, Form::Plain).text;
        let lines: Vec<&str> = drawn.lines().collect();
        assert!(lines.contains(&"┃ ▲ Warning"), "{drawn:?}");
        assert!(lines.contains(&"┃ Body."), "{drawn:?}");
        assert!(!drawn.contains('\x1b'), "{drawn:?}");
    }

    #[test]
    fn a_marker_beyond_the_alerts_is_a_neutral_callout() {
        let drawn = draw_note(&alert_note("FOO"), 80, Form::Colored).text;
        // Neither the bar nor the title has a colour: the first line of the
        // callout is exactly a bar and a bold title.
        assert!(drawn.contains("┃ \x1b[1m○ Foo\x1b[22m\n"), "{drawn:?}");
    }

    /// Guard: escaping runs on every span, a callout's title included.
    #[test]
    fn a_callout_title_is_escaped_like_any_text() {
        let note = "---\ntitle: A\n---\n> [!NOTE] title &#x1b;[2J\n> body\n";
        for form in [Form::Plain, Form::Styled, Form::Colored] {
            let drawn = draw_note(note, 80, form).text;
            assert!(drawn.contains("\\u{1b}[2J"), "{form:?}: {drawn:?}");
            assert!(!without_sgr(&drawn).contains('\x1b'), "{form:?}: {drawn:?}");
        }
    }

    /// Guard: the callout's bar and symbols count one column each, as the
    /// drawn line is.
    #[test]
    fn rows_count_a_callout_by_its_drawn_width() {
        let drawn = draw_note(&alert_note("CAUTION"), 80, Form::Colored);
        assert_eq!(drawn.line_widths, line_widths(&drawn.text));
    }

    /// Guard: a plain quote keeps its own bar beside a callout.
    #[test]
    fn a_quote_keeps_its_bar_beside_a_callout() {
        let note = "---\ntitle: A\n---\n> quoted\n\n> [!NOTE]\n> called\n";
        let drawn = draw_note(note, 80, Form::Plain).text;
        let lines: Vec<&str> = drawn.lines().collect();
        assert!(lines.contains(&"│ quoted"), "{drawn:?}");
        assert!(lines.contains(&"┃ called"), "{drawn:?}");
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

    /// A tab's stop depends on everything before it on the line. Measuring
    /// that again at every tab costs time quadratic in the tabs: a release
    /// build took 3.4 s for 10,000 tabs on one line, where patine's own
    /// layout takes under 10 ms. The limit is far above what measuring each
    /// run of text once takes.
    #[test]
    fn tabs_cost_time_in_proportion_to_their_number() {
        let tabs = 10_000;
        let note = format!("---\ntitle: T\n---\n```\n{}\n```\n", "a\t".repeat(tabs));
        let started = std::time::Instant::now();
        let drawn = draw_note(&note, 80, Form::Plain);
        let elapsed = started.elapsed();
        assert!(drawn.text.contains("a       a"));
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "drawing {tabs} tabs took {elapsed:?}"
        );
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
