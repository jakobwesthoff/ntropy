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

In the search picker, one key selects the current row the way Enter does but
behaves as if `--print` had been given: the path goes to stdout and the editor
does not open.

## Proposal

- `picker::pick` (`src/bin/ntropy/run/picker/mod.rs`) returns only the chosen
  item; `run_loop` maps `KeyCode::Enter` to `state.into_selected()`. It needs to
  also report which accept key ended the picker, for example a small enum (open
  or print) next to the item.
- `cmd_search` (`src/bin/ntropy/run/mod.rs`) branches on it: the print accept
  takes the existing `Some(path) if print` arm, so stdout and the exit code
  match `-p`.
- The other picker callers, `cmd_delete` in `src/bin/ntropy/run/mod.rs` and the
  picker in `src/bin/ntropy/run/render.rs`, either ignore the key or give it a
  meaning of their own.

## Open questions

- Which key: a Ctrl combination that terminals deliver reliably in raw mode and
  that does not collide with the picker's bindings (Ctrl-P, Ctrl-N, Ctrl-U,
  Ctrl-W, Ctrl-C).
- Does the stats line hint at the key, or only the docs?
- How does it combine with `--print-content`?

## Done when

- In the search picker, the key prints the selected note's path to stdout,
  exits 0 and does not open the editor.
- The key is documented with the other picker keys in `docs/design/cli.md`,
  the user-facing picker page in `docs/website/` and ADR 0027.
