# Picker prompt line is not clipped to the terminal width; a long query wraps and breaks the layout

Found during the 2026-07-02 codebase review (unit 11, picker TUI).

## Problem

Every other element the picker draws respects the terminal width: list rows
truncate character-by-character against `cols`
(`src/bin/ntropy/run/picker/mod.rs:277-317`), the dividers are exactly
`cols` wide, and the stats line clips itself. The prompt line does not:

```rust
style::Print(format!("{PROMPT_PREFIX}{}", state.query())),
```

(`src/bin/ntropy/run/picker/mod.rs:196-201`). When the typed query is wider
than the terminal, the line wraps onto the next row, pushing the second
divider and the stats line down and corrupting the fixed bottom-anchored
frame (rows are positioned by absolute `MoveTo`, so the wrapped remnant
overlaps whatever is drawn there next). The cursor parking calculation
(`prompt_col` at `mod.rs:216`) likewise exceeds the width and the terminal
clamps it to the last column.

Reproduce: open the picker in a narrow terminal and type more characters
than the window is wide.

## Suggested fix

Clip the rendered query to `cols - PROMPT_PREFIX.width()` display columns
before printing, keeping the *end* of the query visible (the user is editing
at the end; showing the tail matches what shells/fzf do), and clamp the
parked cursor column to `cols - 1`.

## Acceptance

- With a query wider than the terminal, the frame stays intact (list,
  dividers, stats all at their rows) and the tail of the query is visible.
- A unit test for the clipping helper (display-width aware, like the
  existing `truncate` in `layout.rs` but keeping the tail).
