# 27. In-house fuzzy picker over nucleo and crossterm

Date: 2026-06-25

## Status

Accepted

Supersedes the picker-library part of [ADR 0014](0014-interactive-by-default-cli-with-auto-output-mode.md).

## Context

ADR 0014 embedded the interactive picker with `nucleo-picker`. That crate
hardcodes its selection colors (a `DarkGrey` background) and, as of 0.11.1 (the
latest release), exposes no API to theme them. On a light terminal the fixed
colors give poor contrast, and ntropy cannot adapt the picker to the user's
terminal theme.

## Decision

Render the picker in-house over the `nucleo` matcher (already a dependency) and
`crossterm`, dropping `nucleo-picker`.

- The public surface is one generic function,
  `pick<T>(items, render) -> Result<Option<T>>`, so the engine can be replaced
  without touching call sites. `render` produces each item's display row, which
  is also the fuzzy-match haystack.
- The selection is drawn with reverse video (a full-width bar), which inverts
  the terminal's own colors and so adapts to any light or dark theme. Matched
  characters are bold, located via `nucleo`'s match positions.
- The picker uses the alternate screen at full height, with a prompt line and an
  `m/n` match counter.
- Keybindings: typing, Backspace, Ctrl-W (delete word) and Ctrl-U (clear) edit
  the query; Up / Ctrl-P and Down / Ctrl-N move; Enter selects; Esc and Ctrl-C
  abort.
- All interaction logic lives in a pure `PickerState` that is unit tested
  without a TTY (ADR 0021); the `crossterm` event-and-draw loop is the only
  untested glue.

## Consequences

- The selection adapts to the terminal theme instead of using fixed colors.
- `crossterm` is added; `nucleo-picker` is removed. `nucleo` stays as the
  matcher.
- The terminal loop is not covered by tests, by design; the logic it drives is.
- Matching runs over the full rendered row (title, date, tags), unchanged from
  the previous behaviour.
- The query has no intra-line cursor movement (Left/Right) in v1; edits act at
  the end of the input.
