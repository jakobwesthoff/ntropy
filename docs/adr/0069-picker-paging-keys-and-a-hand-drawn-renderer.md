# 69. Picker paging keys and a hand-drawn renderer

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md),
whose picker moved the selection one row at a time and cleared the query
with Ctrl-U, and [ADR 0060](0060-additional-picker-actions-on-accept-keys.md),
whose reserved Ctrl keys were C, N, P, U and W.

## Context

The picker moved through long result lists one row at a time. Whether it
should draw with the `ratatui` crate instead of its own `crossterm` code was
open.

## Decision

- Ctrl-D moves the selection half the list height toward the best match and
  Ctrl-U half the height toward worse matches. PageDown and PageUp move by
  the full height. Half a page is at least one row.
- The list scrolls by the same number of rows, so the selected row keeps its
  place on the screen until the list reaches an end. There the selection
  stops at the best or the worst match.
- Ctrl-U no longer clears the query. Backspace and Ctrl-W edit it.
- Ctrl-C, Ctrl-D, Ctrl-N, Ctrl-P, Ctrl-U and Ctrl-W are the picker's own; an
  accept key on one of them is ignored.
- The picker keeps drawing itself over `crossterm`. A prototype on `ratatui`
  0.30 drew on the controlling terminal as
  [ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md)
  requires and made the whole frame testable, but it replaced only the
  frame drawing, not the column layout of
  [ADR 0059](0059-width-aware-picker-columns.md); it delivered each frame in
  three writes and showed the cursor before parking it, where the
  hand-drawn frame is one write with the cursor parked first; it split an
  emoji sequence where a match highlight began inside it; and it added 22
  crates to the build.

## Consequences

- The drawn frame stays without unit tests
  ([ADR 0021](0021-testing-strategy-with-insta-across-all-layers.md)). The
  picker state, the key handling and the line helpers are tested without a
  terminal.
