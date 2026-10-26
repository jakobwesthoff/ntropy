---
title: "Add half-page and full-page scrolling to the picker"
kind: feature
component: cli
status: needs-discussion
origin: request
tags: [ux]
---
# Add half-page and full-page scrolling to the picker

Requested by the user on 2026-10-26. The fuzzy picker (`search`, and the
ambiguity pickers of `delete` and `render`) moves the selection one row
at a time with Up/Down and Ctrl-P/Ctrl-N. Long result lists need a
faster way through. The user wants vim's half-page keys for it, Ctrl-D
for half a page down and Ctrl-U for half a page up, plus PageDown and
PageUp for a full page.

## Goal

Ctrl-D and Ctrl-U move the selection by half the visible list height
and scroll the viewport with it, as in vim. PageDown and PageUp do the
same by the full visible height, as in fzf and less. The picker's
layout is bottom-anchored (ADR 0027): the best match sits at the bottom
next to the prompt and worse matches go up the screen. Down (Ctrl-D,
PageDown) therefore moves toward the best match (`select_better`
direction), up (Ctrl-U, PageUp) toward worse matches (`select_worse`
direction).

## Conflict to settle first

Ctrl-U is already taken. It clears the query (readline
`unix-line-discard`, also fzf's default), handled in
`src/bin/ntropy/run/picker/mod.rs` in the key loop of `pick_with_keys`,
and documented in `docs/design/cli.md` (picker section) and ADR 0027.
Before implementing, decide which of these holds:

- Ctrl-U becomes half-page-up and query clearing moves elsewhere, or is
  dropped since Ctrl-W and Backspace remain.
- Ctrl-U stays as clear query and half-page-up gets another key, which
  breaks the vim pairing.

Ctrl-D, PageUp and PageDown are free today.

## Proposal

- `PickerState` in `src/bin/ntropy/run/picker/state.rs` gets one method
  per direction that moves `selected` by a given number of rows,
  clamped to the ends of the match list, followed by
  `scroll_to_selection`. Half a page is `max(1, height / 2)`, a full
  page is `height`. Vim also shifts the viewport by the same amount so
  the cursor keeps its screen row; whether to copy that or only keep
  the selection visible is a detail for the implementation.
- The key loop in `pick_with_keys` maps Ctrl-D, Ctrl-U, PageDown and
  PageUp to them.
- `BUILT_IN_CTRL_KEYS` in `src/bin/ntropy/run/picker/mod.rs` gains `d`,
  so no accept key (ADR 0060) can shadow it. ADR 0060 lists the
  reserved Ctrl keys, so the change needs a new ADR or an amendment.
  PageUp and PageDown are not Ctrl letters and never reach
  `accept_key`.
- Update the key lists in ADR 0027 (or its successor) and
  `docs/design/cli.md`, and the ntropy skill if it lists picker keys.
- Unit tests in `state.rs` for half-page and full-page movement,
  clamping at both ends, and a viewport of height 1.

## Decisions

- 2026-10-26, user: PageUp and PageDown are part of this todo.
