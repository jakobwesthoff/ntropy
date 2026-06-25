// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The interactive fuzzy picker (ADR 0014, ADR 0027).
//!
//! ntropy renders its own picker over the `nucleo` matcher and `crossterm`
//! rather than a picker library, so the selection bar adapts to any terminal
//! theme (it uses reverse video instead of a hardcoded background) and the
//! whole UI stays under our control. The public surface is the single generic
//! [`pick`] function; everything else is private, so swapping the engine never
//! touches call sites.
//!
//! All interaction logic lives in [`state::PickerState`] and is unit tested
//! without a TTY (ADR 0021). This module is the thin glue that maps `crossterm`
//! key events onto that state and draws it.

mod layout;
mod state;

use std::io::{self, Write};

use anyhow::{Context, Result};
use crossterm::style::Attribute;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue, style, terminal,
};

pub use layout::align_candidates;
use state::{PickerState, VisibleRow};

/// One picker row split into its matchable and display-only parts.
///
/// The fuzzy matcher and the match-highlighting run over `matchable` only;
/// `suffix` is shown (dimmed) but never matched, so a long identifier can be
/// visible without polluting the query or the highlight.
pub struct Row {
    /// The matchable, highlightable text (shown first).
    pub matchable: String,
    /// Trailing display-only text, e.g. a note's ULID.
    pub suffix: String,
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

    let mut stdout = io::stdout();
    terminal::enable_raw_mode().context("while enabling raw mode")?;
    execute!(stdout, terminal::EnterAlternateScreen)
        .context("while entering the alternate screen")?;
    // The guard restores the terminal on every exit path, including `?` and
    // panics, so a failure mid-loop never leaves the user in raw mode.
    let _guard = TerminalGuard;

    run_loop(&mut stdout, items, render_all)
}

/// Restores the terminal to its normal mode when the picker exits.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

/// The read-draw-react loop. Returns the selection, or `None` on abort.
fn run_loop<T>(
    stdout: &mut io::Stdout,
    items: Vec<T>,
    render_all: impl FnOnce(&[T]) -> Vec<Row>,
) -> Result<Option<T>> {
    let (mut cols, rows) = terminal::size().context("while querying the terminal size")?;
    let picker_rows = render_all(&items);
    let mut state = PickerState::new(items, picker_rows, list_height(rows));

    loop {
        draw(stdout, &state, cols).context("while drawing the picker")?;

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

/// The number of list rows that fit below the prompt and counter lines.
fn list_height(terminal_rows: u16) -> usize {
    (terminal_rows as usize).saturating_sub(2).max(1)
}

/// Draw the whole picker, bottom-anchored: the list region (best match at the
/// bottom), a divider carrying the counter, then the prompt on the last line.
fn draw<T>(stdout: &mut io::Stdout, state: &PickerState<T>, cols: u16) -> Result<()> {
    queue!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::All),
    )?;

    // The list region is exactly `height` lines; blanks (`None`) at the top are
    // left clear so the rows hug the divider and prompt below them.
    let lines = state.list_lines();
    let list_rows = lines.len() as u16;
    for (i, line) in lines.iter().enumerate() {
        if let Some(row) = line {
            queue!(stdout, cursor::MoveTo(0, i as u16))?;
            draw_row(stdout, row, cols)?;
        }
    }

    // Divider with the counter, directly above the prompt.
    let (matching, total) = state.counter();
    queue!(stdout, cursor::MoveTo(0, list_rows))?;
    draw_divider(stdout, cols, matching, total)?;

    // Prompt pinned to the bottom line; park the cursor at its end so typing
    // reads naturally.
    let prompt_row = list_rows + 1;
    queue!(
        stdout,
        cursor::MoveTo(0, prompt_row),
        style::Print(format!("❯ {}", state.query())),
    )?;
    let prompt_col = 2 + state.query().chars().count() as u16;
    queue!(stdout, cursor::MoveTo(prompt_col, prompt_row))?;

    stdout.flush().context("while flushing the picker frame")?;
    Ok(())
}

/// Draw the divider line that separates the list from the prompt, with the
/// `m/n` counter right-aligned near the right edge.
fn draw_divider(stdout: &mut io::Stdout, cols: u16, matching: usize, total: usize) -> Result<()> {
    queue!(
        stdout,
        style::Print(divider_line(cols as usize, matching, total, '─')),
    )?;
    Ok(())
}

/// Build the divider string: a run of `fill`, the counter right-aligned with one
/// trailing `fill`, padded to `width`. Narrower than the counter, it degrades to
/// just the (truncated) counter.
fn divider_line(width: usize, matching: usize, total: usize, fill: char) -> String {
    let label = format!(" {matching}/{total} ");
    let label_len = label.chars().count();
    if width <= label_len {
        return label.chars().take(width).collect();
    }
    // One trailing fill keeps the counter off the very edge; the rest leads.
    let leading = width - label_len - 1;
    let mut line: String = std::iter::repeat_n(fill, leading).collect();
    line.push_str(&label);
    line.push(fill);
    line
}

/// Draw a single list row. The selected row is drawn in cyan with a `❯ `
/// pointer; matched characters are yellow on either row; the display-only ULID
/// suffix is dimmed (or rides the cyan body on the selected row). All colors are
/// the terminal's own ANSI palette, so the picker adapts to its theme.
fn draw_row(stdout: &mut io::Stdout, row: &VisibleRow<'_>, cols: u16) -> Result<()> {
    let width = cols as usize;
    let selected = row.selected;
    let pointer = if selected { "❯ " } else { "  " };

    // The selected row's body is cyan; the pointer shares that accent.
    if selected {
        queue!(stdout, style::SetForegroundColor(style::Color::Cyan))?;
    }
    queue!(stdout, style::Print(pointer))?;

    // Truncate to the terminal width so a long row never wraps and breaks the
    // layout. The matchable part is drawn first (matches in yellow), then the
    // suffix fills whatever budget remains. `❯ ` is two display columns.
    let mut drawn = pointer.chars().count();
    let positions = row.positions;
    for (i, c) in row.matchable.chars().enumerate() {
        if drawn >= width {
            break;
        }
        let matched = positions.binary_search(&(i as u32)).is_ok();
        if matched {
            queue!(stdout, style::SetForegroundColor(style::Color::Yellow))?;
        }
        queue!(stdout, style::Print(c))?;
        if matched {
            // Restore the row's base color after a highlighted character.
            if selected {
                queue!(stdout, style::SetForegroundColor(style::Color::Cyan))?;
            } else {
                queue!(stdout, style::ResetColor)?;
            }
        }
        drawn += 1;
    }

    if !row.suffix.is_empty() && drawn < width {
        // The ULID is dimmed on unselected rows; on the selected row it simply
        // rides the cyan body color.
        if !selected {
            queue!(stdout, style::SetAttribute(Attribute::Dim))?;
        }
        for c in row.suffix.chars() {
            if drawn >= width {
                break;
            }
            queue!(stdout, style::Print(c))?;
            drawn += 1;
        }
        if !selected {
            queue!(stdout, style::SetAttribute(Attribute::NormalIntensity))?;
        }
    }

    // Reset styling so it never bleeds into the next line or a blank area.
    queue!(
        stdout,
        style::SetAttribute(Attribute::Reset),
        style::ResetColor
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divider_right_aligns_the_counter_with_one_trailing_fill() {
        let line = divider_line(20, 12, 40, '-');
        assert_eq!(line.chars().count(), 20);
        assert!(line.ends_with('-'));
        assert!(line.contains(" 12/40 "));
        // The counter hugs the right edge: only one fill follows it.
        assert!(line.ends_with("12/40 -"));
        assert!(line.starts_with("------"));
    }

    #[test]
    fn divider_degrades_to_the_counter_when_too_narrow() {
        // Width below the counter label just shows a truncated counter.
        let line = divider_line(4, 1, 2, '-');
        assert_eq!(line.chars().count(), 4);
        assert!(!line.contains('-'));
    }

    #[test]
    fn divider_fills_the_full_width() {
        let line = divider_line(30, 3, 3, '─');
        assert_eq!(line.chars().count(), 30);
        assert!(line.starts_with('─'));
        assert!(line.contains(" 3/3 "));
    }
}
