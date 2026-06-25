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
                    KeyCode::Up => state.move_up(),
                    KeyCode::Down => state.move_down(),
                    KeyCode::Char('p') if ctrl => state.move_up(),
                    KeyCode::Char('n') if ctrl => state.move_down(),
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

/// Draw the whole picker: prompt, counter, then the visible rows.
fn draw<T>(stdout: &mut io::Stdout, state: &PickerState<T>, cols: u16) -> Result<()> {
    queue!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::All),
        style::Print(format!("> {}", state.query())),
        cursor::MoveToNextLine(1),
    )?;

    let (matching, total) = state.counter();
    queue!(
        stdout,
        style::Print(format!("{matching}/{total}")),
        cursor::MoveToNextLine(1),
    )?;

    for row in state.visible() {
        draw_row(stdout, &row, cols)?;
        queue!(stdout, cursor::MoveToNextLine(1))?;
    }

    // Park the cursor at the end of the query so typing reads naturally.
    let prompt_col = 2 + state.query().chars().count() as u16;
    queue!(stdout, cursor::MoveTo(prompt_col, 0))?;

    stdout.flush().context("while flushing the picker frame")?;
    Ok(())
}

/// Draw a single list row: a `> ` pointer for the selection, matched characters
/// in the matchable part bold, the display-only suffix dimmed, and (for the
/// selection) a reverse-video bar padded to the full width so it reads as a
/// highlighted line on any terminal theme.
fn draw_row(stdout: &mut io::Stdout, row: &VisibleRow<'_>, cols: u16) -> Result<()> {
    let width = cols as usize;
    if row.selected {
        queue!(stdout, style::SetAttribute(Attribute::Reverse))?;
    }

    let pointer = if row.selected { "> " } else { "  " };
    queue!(stdout, style::Print(pointer))?;

    // Truncate to the terminal width so a long row never wraps and breaks the
    // layout. The matchable part is drawn first (highlighted), then the dimmed
    // suffix fills whatever budget remains.
    let mut drawn = pointer.len();
    let positions = row.positions;
    for (i, c) in row.matchable.chars().enumerate() {
        if drawn >= width {
            break;
        }
        let matched = positions.binary_search(&(i as u32)).is_ok();
        if matched {
            queue!(stdout, style::SetAttribute(Attribute::Bold))?;
        }
        queue!(stdout, style::Print(c))?;
        if matched {
            queue!(stdout, style::SetAttribute(Attribute::NormalIntensity))?;
        }
        drawn += 1;
    }

    if !row.suffix.is_empty() && drawn < width {
        queue!(stdout, style::SetAttribute(Attribute::Dim))?;
        for c in row.suffix.chars() {
            if drawn >= width {
                break;
            }
            queue!(stdout, style::Print(c))?;
            drawn += 1;
        }
        queue!(stdout, style::SetAttribute(Attribute::NormalIntensity))?;
    }

    if row.selected {
        // Pad the reverse-video bar across the rest of the line.
        if drawn < width {
            let pad: String = " ".repeat(width - drawn);
            queue!(stdout, style::Print(pad))?;
        }
        queue!(stdout, style::SetAttribute(Attribute::Reset))?;
    }
    Ok(())
}
