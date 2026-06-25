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
  `pick<T>(items, render_all) -> Result<Option<T>>`, so the engine can be
  replaced without touching call sites. `render_all` turns the whole item set
  into its rows in one pass so columns can be aligned across every candidate;
  each row's matchable text is also its fuzzy-match haystack.
- The layout is bottom-anchored: the prompt is framed by a (blue) divider line
  above and below it, with a dimmed stats line beneath the lower divider,
  indented to sit under the query text and showing the cursor's rank within the
  matches, the match count and the total (or an empty-state hint); the result
  list grows upward above the top divider with the best match nearest the
  prompt. Rows are an aligned grid of title, date and tags; the ULID trails as a
  dimmed, display-only suffix.
- Matched characters are drawn in yellow and the selected row in cyan with a
  `▌` bar. The colors come from the terminal's own ANSI palette, so the
  picker adapts to any light or dark theme. Match positions come from `nucleo`.
- Keybindings: typing, Backspace, Ctrl-W (delete word) and Ctrl-U (clear) edit
  the query; Up / Ctrl-P move toward worse matches (up the screen), Down /
  Ctrl-N toward the best (down toward the prompt); Enter selects; Esc and
  Ctrl-C abort.
- All interaction logic lives in a pure `PickerState` (filtering, selection,
  the bottom-up `list_lines`) plus a pure `align_candidates`, both unit tested
  without a TTY (ADR 0021); the `crossterm` event-and-draw loop is the only
  untested glue.

## Consequences

- Match (yellow) and selection (cyan) colors are themeable ANSI palette
  entries, so the picker adapts to the terminal theme instead of using fixed
  colors.
- `crossterm` is added; `nucleo-picker` is removed. `nucleo` stays as the
  matcher.
- The terminal loop is not covered by tests, by design; the logic it drives is.
- Matching and highlighting run over the matchable part of a row (title, date,
  tags). A trailing display-only suffix (the note's ULID) is shown dimmed but
  never matched.
- Column widths come from absolute caps (title 48, tags 32 chars, ellipsis
  truncation), so the grid is stable across a resize; only the per-line
  truncation at the terminal width reacts to it.
- The query has no intra-line cursor movement (Left/Right) in v1; edits act at
  the end of the input.
