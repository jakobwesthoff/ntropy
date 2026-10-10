---
title: "Decide whether the picker should render with ratatui"
kind: investigation
component: cli
status: needs-discussion
origin: request
tags: [ux]
---
# Decide whether the picker should render with ratatui

Requested by the user on 2026-10-10, after the picker's column sizing was
reworked (ADR 0059). The picker draws itself by hand over `crossterm` (ADR
0027): `src/bin/ntropy/run/picker/mod.rs` queues every frame, brackets it in a
synchronized update, erases per line, hides and parks the cursor, and clips
each row to the terminal width, while `picker/layout.rs` computes the column
widths. Each new piece of UI (the stats line, a prompt that clips, a second
accept key) is more hand-drawn code. This investigation settles whether moving
the rendering to the `ratatui` crate is worth it.

## Context

Constraints any replacement has to keep:

- The picker draws on the controlling terminal, never on stdout, so
  `ntropy search -p | pbcopy` still pipes only the chosen path (ADR 0036).
- All interaction logic lives in the pure `PickerState`, unit tested and
  snapshot tested without a TTY (ADR 0021, ADR 0027); the terminal loop is
  the only untested glue.
- Colors come from the terminal's own ANSI palette, so the picker follows
  the user's theme.
- Rows are measured with the same string width everywhere, so emoji
  sequences line up (`layout::fit_prefix`).
- `crossterm` 0.29 and `nucleo` 0.5 are the current dependencies.

## Options

- Keep the hand-written renderer and extend it as needed.
- Render with `ratatui` over its `crossterm` backend, keeping `PickerState`
  and `nucleo` as they are.

## Open questions

- Can `ratatui` draw on the controlling terminal while stdout is a pipe, and
  does it support the alternate screen and synchronized updates the picker
  uses now?
- Which hand-written parts would it replace: the frame loop, line erasing and
  cursor handling, row clipping, the column layout?
- How does its width measurement treat emoji ZWJ and VS16 sequences compared
  with `layout::fit_prefix`?
- What does it add to build time and binary size?
- Does its test backend give tests at least as good as the current
  `debug_render` snapshots?

## Relations

- Relates to: [Picker prompt line is not clipped to the terminal width](01KWH6QRNAMFX7MM7GT6NM4CJ0-picker-prompt-line-is-not-clipped-to-the-terminal-width.md), a framework would clip the prompt
- Relates to: [Print the selected note's path from the search picker with a key](01M4JTGFFE0A5JZ9K9MENZZ90X-print-the-selected-notes-path-from-the-search-picker-with-a-key.md), adds to the same event loop
