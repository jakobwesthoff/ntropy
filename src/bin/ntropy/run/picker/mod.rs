// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The interactive fuzzy picker (ADR 0027, ADR 0036).
//!
//! ntropy renders its own picker over the `nucleo` matcher and `crossterm`
//! rather than a picker library, so the selection bar adapts to any terminal
//! theme (it uses reverse video instead of a hardcoded background) and the
//! whole UI stays under our control. It draws on the controlling terminal,
//! keeping stdout free for piped data (ADR 0036). The public surface is the
//! single generic [`pick`] function; everything else is private, so swapping
//! the engine never touches call sites.
//!
//! All interaction logic lives in [`state::PickerState`] and is unit tested
//! without a TTY (ADR 0021). This module is the thin glue that maps `crossterm`
//! key events onto that state and draws it.

mod layout;
mod state;

use std::fs::File;
use std::io::Write;

use anyhow::{Context, Result};
use crossterm::style::Attribute;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue, style, terminal,
};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use layout::align_candidates;
use state::{PickerState, VisibleRow};

/// One picker row split by role: searched, displayed, and display-only.
///
/// Fuzzy scoring runs over `search` (the full content, which may extend past
/// what the width-capped columns can show); match-highlighting runs over
/// `display`; `suffix` is shown (dimmed) but never matched, so a long
/// identifier can be visible without polluting the query or the highlight.
pub struct Row {
    /// The displayed, highlightable text (shown first).
    pub display: String,
    /// Trailing display-only text, e.g. a note's ULID.
    pub suffix: String,
    /// The full content scored against the query, untruncated and unpadded.
    pub search: String,
}

/// Present `items` in the interactive picker and return the chosen one.
///
/// `render_all` turns the whole item set into its [`Row`]s in one pass, which
/// lets the renderer align columns across every candidate (see [`layout`]).
/// Returns `Ok(None)` when there are no items or the user aborts (Esc / Ctrl-C)
/// without selecting.
pub fn pick<T>(items: Vec<T>, render_all: impl FnOnce(&[T]) -> Vec<Row>) -> Result<Option<T>> {
    // Nothing to pick: never touch the terminal so non-interactive callers and
    // empty result sets stay side-effect free.
    if items.is_empty() {
        return Ok(None);
    }

    // The picker draws on the controlling terminal, never on stdout, so stdout
    // stays a pure data channel: `search -p | pbcopy` shows the picker and
    // pipes only the selected path (ADR 0036).
    let mut tty = super::interact::open_tty()
        .context("while opening the controlling terminal for the picker")?;
    let mut teardown_tty = tty
        .try_clone()
        .context("while cloning the terminal handle")?;
    terminal::enable_raw_mode().context("while enabling raw mode")?;
    // Arm raw-mode teardown the instant raw mode is on, before any further
    // fallible setup. If entering the alternate screen below fails, this guard
    // still restores the terminal, so the user's shell never stays in raw mode.
    let _raw_guard = TerminalGuard::new(|| {
        let _ = terminal::disable_raw_mode();
    });
    execute!(tty, terminal::EnterAlternateScreen).context("while entering the alternate screen")?;
    // Armed only after the alternate screen is actually entered, so it leaves
    // exactly what was entered. It drops before the raw guard (reverse arming
    // order), leaving the alternate screen before raw mode is disabled.
    let _alt_guard = TerminalGuard::new(move || {
        let _ = execute!(teardown_tty, terminal::LeaveAlternateScreen, cursor::Show);
    });

    run_loop(&mut tty, items, render_all)
}

/// Restores one piece of terminal state when dropped, on every exit path
/// (`?`, normal return, panic). One guard is armed per setup step so a failure
/// between steps still tears down everything already set up.
///
/// The teardown action is injected rather than hardcoded so the arming-order
/// guarantee is unit-testable without a real terminal.
struct TerminalGuard<F: FnMut()> {
    teardown: F,
}

impl<F: FnMut()> TerminalGuard<F> {
    fn new(teardown: F) -> Self {
        Self { teardown }
    }
}

impl<F: FnMut()> Drop for TerminalGuard<F> {
    fn drop(&mut self) {
        (self.teardown)();
    }
}

/// The read-draw-react loop. Returns the selection, or `None` on abort.
fn run_loop<T>(
    tty: &mut File,
    items: Vec<T>,
    render_all: impl FnOnce(&[T]) -> Vec<Row>,
) -> Result<Option<T>> {
    let (mut cols, rows) = terminal::size().context("while querying the terminal size")?;
    let picker_rows = render_all(&items);
    let mut state = PickerState::new(items, picker_rows, list_height(rows));

    loop {
        draw(tty, &state, cols).context("while drawing the picker")?;

        match event::read().context("while reading a key event")? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Enter => return Ok(state.into_selected()),
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if ctrl => return Ok(None),
                    // The list is bottom-anchored with the best match at the
                    // bottom, so Up moves toward worse matches (up the screen)
                    // and Down toward the best (down toward the prompt).
                    KeyCode::Up => state.select_worse(),
                    KeyCode::Down => state.select_better(),
                    KeyCode::Char('p') if ctrl => state.select_worse(),
                    KeyCode::Char('n') if ctrl => state.select_better(),
                    KeyCode::Char('u') if ctrl => state.clear_query(),
                    KeyCode::Char('w') if ctrl => state.delete_word(),
                    KeyCode::Backspace => state.backspace(),
                    // Plain typing (no Ctrl/Alt) edits the query. TODO: no
                    // intra-query cursor movement (Left/Right) in v1.
                    KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                        state.push_char(c)
                    }
                    _ => {}
                }
            }
            Event::Resize(new_cols, new_rows) => {
                cols = new_cols;
                state.set_height(list_height(new_rows));
            }
            _ => {}
        }
    }
}

/// The color of the divider lines that frame the prompt.
const DIVIDER_COLOR: style::Color = style::Color::Blue;

/// The prompt prefix; its width is also the indent the query text and the stats
/// line share, so the stats sit directly under the query.
const PROMPT_PREFIX: &str = "❯ ";

/// The selected-row marker. A left bar reads as a selection gutter and stays
/// distinct from the prompt's `❯`. Two columns wide, like the unselected `  `.
const SELECTION_POINTER: &str = "▌ ";

/// The number of list rows that fit above the divider/prompt/divider/stats chrome.
fn list_height(terminal_rows: u16) -> usize {
    (terminal_rows as usize).saturating_sub(4).max(1)
}

/// Draw the whole picker, bottom-anchored: the list region (best match at the
/// bottom), a divider, the prompt, a second divider, then the dimmed `m/n` stats.
fn draw<T>(tty: &mut File, state: &PickerState<T>, cols: u16) -> Result<()> {
    queue!(
        tty,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::All),
    )?;

    // The list region is exactly `height` lines; blanks (`None`) at the top are
    // left clear so the rows hug the divider and prompt below them.
    let lines = state.list_lines();
    let list_rows = lines.len() as u16;
    for (i, line) in lines.iter().enumerate() {
        if let Some(row) = line {
            queue!(tty, cursor::MoveTo(0, i as u16))?;
            draw_row(tty, row, cols)?;
        }
    }

    // Divider directly above the prompt.
    queue!(tty, cursor::MoveTo(0, list_rows))?;
    draw_divider(tty, cols)?;

    // Prompt, framed by a second divider below it.
    let prompt_row = list_rows + 1;
    queue!(
        tty,
        cursor::MoveTo(0, prompt_row),
        style::Print(format!("{PROMPT_PREFIX}{}", state.query())),
        cursor::MoveTo(0, prompt_row + 1),
    )?;
    draw_divider(tty, cols)?;

    // Dimmed stats under the second divider, aligned under the query text.
    let (matching, total) = state.counter();
    let rank = state.selected_rank();
    queue!(
        tty,
        cursor::MoveTo(0, prompt_row + 2),
        style::SetAttribute(Attribute::Dim),
        style::Print(stats_line(cols as usize, rank, matching, total)),
        style::SetAttribute(Attribute::Reset),
    )?;

    // Park the cursor at the end of the query so typing reads naturally.
    let prompt_col = (PROMPT_PREFIX.width() + state.query().width()) as u16;
    queue!(tty, cursor::MoveTo(prompt_col, prompt_row))?;

    tty.flush().context("while flushing the picker frame")?;
    Ok(())
}

/// Draw a full-width colored divider line.
fn draw_divider(tty: &mut File, cols: u16) -> Result<()> {
    queue!(
        tty,
        style::SetForegroundColor(DIVIDER_COLOR),
        style::Print(divider_line(cols as usize, '─')),
        style::ResetColor,
    )?;
    Ok(())
}

/// A run of `fill` exactly `width` columns wide.
fn divider_line(width: usize, fill: char) -> String {
    std::iter::repeat_n(fill, width).collect()
}

/// The dimmed stats string, indented to sit directly under the query text (past
/// the prompt prefix). Shows the cursor's rank within the matches plus the total
/// candidate count, or an empty-state hint. Clipped to `width` on a narrow
/// terminal. `rank` is the 1-based position of the selection among the matches,
/// or `None` when nothing matches.
fn stats_line(width: usize, rank: Option<usize>, matching: usize, total: usize) -> String {
    let body = match rank {
        None => format!("no matches · {total} total"),
        Some(rank) => format!("{rank}/{matching} · {total} total"),
    };
    let mut line = " ".repeat(PROMPT_PREFIX.width());
    line.push_str(&body);
    if line.chars().count() > width {
        return line.chars().take(width).collect();
    }
    line
}

/// Draw a single list row. The selected row is drawn in cyan with a `▌ ` bar;
/// matched characters are yellow on either row; the display-only ULID suffix is
/// dimmed (or rides the cyan body on the selected row). All colors are the
/// terminal's own ANSI palette, so the picker adapts to its theme.
fn draw_row(tty: &mut File, row: &VisibleRow<'_>, cols: u16) -> Result<()> {
    let width = cols as usize;
    let selected = row.selected;
    let pointer = if selected { SELECTION_POINTER } else { "  " };

    // The selected row's body is cyan; the pointer shares that accent.
    if selected {
        queue!(tty, style::SetForegroundColor(style::Color::Cyan))?;
    }
    queue!(tty, style::Print(pointer))?;

    // Truncate to the terminal width (in display columns) so a long row never
    // wraps and breaks the layout. The matchable part is drawn first (matches in
    // yellow), then the suffix fills whatever budget remains. The `▌ ` bar is two
    // display columns; a wide character is dropped whole rather than allowed to
    // straddle the right edge.
    let mut drawn = UnicodeWidthStr::width(pointer);
    let highlights = row.highlights;
    for (i, c) in row.display.chars().enumerate() {
        let w = c.width().unwrap_or(0);
        if drawn + w > width {
            break;
        }
        let matched = highlights.binary_search(&(i as u32)).is_ok();
        if matched {
            queue!(tty, style::SetForegroundColor(style::Color::Yellow))?;
        }
        queue!(tty, style::Print(c))?;
        if matched {
            // Restore the row's base color after a highlighted character.
            if selected {
                queue!(tty, style::SetForegroundColor(style::Color::Cyan))?;
            } else {
                queue!(tty, style::ResetColor)?;
            }
        }
        drawn += w;
    }

    if !row.suffix.is_empty() && drawn < width {
        // The ULID is dimmed on unselected rows; on the selected row it simply
        // rides the cyan body color.
        if !selected {
            queue!(tty, style::SetAttribute(Attribute::Dim))?;
        }
        for c in row.suffix.chars() {
            let w = c.width().unwrap_or(0);
            if drawn + w > width {
                break;
            }
            queue!(tty, style::Print(c))?;
            drawn += w;
        }
        if !selected {
            queue!(tty, style::SetAttribute(Attribute::NormalIntensity))?;
        }
    }

    // Reset styling so it never bleeds into the next line or a blank area.
    queue!(
        tty,
        style::SetAttribute(Attribute::Reset),
        style::ResetColor
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn guards_tear_down_in_reverse_arming_order() {
        let log = Rc::new(RefCell::new(Vec::new()));
        {
            let raw_log = Rc::clone(&log);
            let _raw = TerminalGuard::new(move || raw_log.borrow_mut().push("raw"));
            let alt_log = Rc::clone(&log);
            let _alt = TerminalGuard::new(move || alt_log.borrow_mut().push("alt"));
        }
        // The alternate screen is left before raw mode is disabled.
        assert_eq!(*log.borrow(), vec!["alt", "raw"]);
    }

    #[test]
    fn raw_guard_restores_when_the_alt_screen_step_is_skipped() {
        // Models `EnterAlternateScreen` failing: the alt guard is never armed,
        // yet raw mode must still be restored.
        let log = Rc::new(RefCell::new(Vec::new()));
        {
            let raw_log = Rc::clone(&log);
            let _raw = TerminalGuard::new(move || raw_log.borrow_mut().push("raw"));
        }
        assert_eq!(*log.borrow(), vec!["raw"]);
    }

    #[test]
    fn divider_fills_the_full_width_with_the_glyph() {
        let line = divider_line(30, '─');
        assert_eq!(line.chars().count(), 30);
        assert!(line.chars().all(|c| c == '─'));
    }

    #[test]
    fn divider_of_zero_width_is_empty() {
        assert_eq!(divider_line(0, '─'), "");
    }

    #[test]
    fn stats_align_under_the_query_text() {
        let line = stats_line(40, Some(3), 12, 40);
        // Indented past the prompt prefix so it sits under the query text.
        assert_eq!(line, "  3/12 · 40 total");
        assert_eq!(line.trim_start(), "3/12 · 40 total");
    }

    #[test]
    fn stats_show_an_empty_state_when_nothing_matches() {
        assert_eq!(stats_line(40, None, 0, 40), "  no matches · 40 total");
    }

    #[test]
    fn stats_degrade_to_a_truncation_when_too_narrow() {
        let line = stats_line(4, Some(1), 100, 200);
        assert_eq!(line.chars().count(), 4);
    }
}
