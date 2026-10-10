---
title: "Print the selected note's path from the search picker with a key"
kind: feature
component: cli
horizon: next
origin: request
tags: [ux]
---
# Print the selected note's path from the search picker with a key

Requested by the user on 2026-10-10. `ntropy search` opens the note chosen in
the picker in the editor, unless `-p`/`--print` was given, in which case it
prints the note's path to stdout instead (ADR 0035). The choice is fixed before
the picker opens. When the user forgets `-p` but only wants the path, which
happens often, the only way out is to cancel and run the search again. A second
accept key in the picker, such as a Ctrl combination, would select the
highlighted note and print its path exactly as `-p` does.

## Goal

In the search picker, Ctrl-Y selects the current row the way Enter does but
behaves as if `--print` had been given: the path goes to stdout and the editor
does not open. The stats line gains a right-aligned area of key hints, where
Ctrl-Y is the first entry.

## Proposal

- `picker::pick` (`src/bin/ntropy/run/picker/mod.rs`) returns only the chosen
  item; `run_loop` maps `KeyCode::Enter` to `state.into_selected()`. It needs to
  also report which accept key ended the picker, for example a small enum (open
  or print) next to the item.
- `cmd_search` (`src/bin/ntropy/run/mod.rs`) branches on it: the print accept
  takes the existing `Some(path) if print` arm, so stdout and the exit code
  match `-p`.
- The other picker callers, `cmd_delete` in `src/bin/ntropy/run/mod.rs` and the
  picker in `src/bin/ntropy/run/render.rs`, ignore the key. Each caller tells
  `pick` which extra keys it accepts, so only the search picker shows the hint.
- `stats_line` (`src/bin/ntropy/run/picker/mod.rs`) builds the left part as
  now (counts, then the selected note's ULID) and right-aligns the key hints,
  for example `^Y path`. The hints are the first to go when the line is too
  narrow; the counts and the ULID keep their current order and clipping.

## Decisions

- 2026-10-10, user: the key is Ctrl-Y.
- 2026-10-10, user: only the search picker reacts to it; delete and render
  ignore it.
- 2026-10-10, user: the key is documented in the docs and on the website, and
  hinted in the stats line, in a right-aligned area meant to hold further keys
  later. That area is dropped first when space runs out.

## Open questions

- How does Ctrl-Y combine with `--print-content`? The simplest rule is that it
  always prints the path.

## Done when

- In the search picker, Ctrl-Y prints the selected note's path to stdout,
  exits 0 and does not open the editor. In the delete and render pickers it
  does nothing.
- The search picker's stats line shows the right-aligned Ctrl-Y hint, which
  disappears before the counts or the ULID are cut on a narrow terminal.
- The key is documented with the other picker keys in `docs/design/cli.md`,
  the user-facing picker page in `docs/website/` and ADR 0027.
