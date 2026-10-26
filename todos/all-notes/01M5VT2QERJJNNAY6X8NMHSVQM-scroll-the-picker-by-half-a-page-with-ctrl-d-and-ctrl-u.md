---
title: "Scroll the picker by half a page with Ctrl-D and Ctrl-U"
kind: feature
component: cli
status: needs-discussion
origin: request
tags: [ux]
---
# Scroll the picker by half a page with Ctrl-D and Ctrl-U

Requested by the user on 2026-10-26. The fuzzy picker (`search`, and the
ambiguity pickers of `delete` and `render`) moves the selection one row
at a time with Up/Down and Ctrl-P/Ctrl-N. Long result lists need a
faster way through, and the user wants vim's half-page keys for it:
Ctrl-D for half a page down, Ctrl-U for half a page up.

## Goal

Ctrl-D and Ctrl-U move the selection by half the visible list height
and scroll the viewport with it, as in vim. The picker's layout is
bottom-anchored (ADR 0027): the best match sits at the bottom next to
the prompt and worse matches go up the screen. Half a page down
therefore moves toward the best match (`select_better` direction),
half a page up toward worse matches (`select_worse` direction).

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

Ctrl-D is free today.

## Proposal

- `PickerState` in `src/bin/ntropy/run/picker/state.rs` gets two
  methods that move `selected` by `max(1, height / 2)`, clamped to the
  ends of the match list, followed by `scroll_to_selection`. Vim also
  shifts the viewport by the same amount so the cursor keeps its screen
  row; whether to copy that or only keep the selection visible is a
  detail for the implementation.
- The key loop in `pick_with_keys` maps Ctrl-D and Ctrl-U to them.
- `BUILT_IN_CTRL_KEYS` in `src/bin/ntropy/run/picker/mod.rs` gains `d`,
  so no accept key (ADR 0060) can shadow it. ADR 0060 lists the
  reserved Ctrl keys, so the change needs a new ADR or an amendment.
- Update the key list in `docs/design/cli.md` and the ntropy skill if
  it lists picker keys.
- Unit tests in `state.rs` for the movement and clamping at both ends,
  and for a viewport of height 1.

## Open questions

- Should PageUp and PageDown do the same (or a full page) while this is
  being touched?
