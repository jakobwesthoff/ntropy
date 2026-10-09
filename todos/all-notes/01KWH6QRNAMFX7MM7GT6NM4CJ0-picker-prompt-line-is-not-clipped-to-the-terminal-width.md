---
title: "Picker prompt line is not clipped to the terminal width"
kind: bug
component: cli
origin: review
tags: [ux]
---
# Picker prompt line is not clipped to the terminal width

Found in the 2026-07-02 codebase review of the picker TUI. Every other element the picker draws respects the terminal width: list rows truncate to `cols`, the dividers are `cols` wide, and the stats line clips itself. The prompt line does not, so a query wider than the terminal wraps onto the next row and corrupts the bottom-anchored frame.

## Problem

Read in the code during the review. The prompt is printed without clipping (`src/bin/ntropy/run/picker/mod.rs`, around lines 196 to 201):

```rust
style::Print(format!("{PROMPT_PREFIX}{}", state.query())),
```

Rows are positioned by absolute `MoveTo`, so the wrapped remnant overlaps whatever is drawn at that position next. The cursor parking column (`prompt_col`, around line 216) also exceeds the width, and the terminal clamps it to the last column.

To reproduce, open the picker in a narrow terminal and type more characters than the window is wide.

## Impact

While the query is wider than the terminal, the frame breaks: the second divider and the stats line are pushed down or overwritten.

## Suggested fix

Clip the rendered query to `cols` minus the display width of `PROMPT_PREFIX` before printing. Keep the end of the query visible, since the user edits at the end, as shells and fzf do. Clamp the parked cursor column to `cols - 1`.

## Done when

- With a query wider than the terminal, the frame stays intact (list, dividers and stats at their rows) and the end of the query is visible.
- A unit test covers the clipping helper. It is display-width aware, like the existing `truncate` in `layout.rs`, and keeps the tail.
