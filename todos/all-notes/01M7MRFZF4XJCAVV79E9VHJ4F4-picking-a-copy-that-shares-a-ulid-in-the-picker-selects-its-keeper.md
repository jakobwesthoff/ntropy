---
title: "Picking a copy that shares a ULID in the picker selects its keeper"
kind: bug
component: cli
impact: low
origin: review
---
# Picking a copy that shares a ULID in the picker selects its keeper

When `render` or `show` resolves to a note and a copy carrying the same
ULID and opens the interactive picker, choosing the copy acts on the
keeper. The picked candidate is mapped back to a note by its id, which both
files share. `delete` maps the pick back by path and is not affected. Found
on 2026-11-17 by a plan reviewer of the shared-ULID hint fix (todo
01M7MQDTR6RZW9HDFG4WNDCAWB), confirmed by reading the code; not reproduced
interactively.

## Problem

`src/bin/ntropy/run/one_note.rs`, the multi-match arm of the selection:
`notes.iter().find(|n| n.id == selected.id)` returns the first note with
that id, the keeper, whichever candidate was picked.

## Suggested fix

Map the picked candidate back by path, as `delete` does, with a test that
picks the copy.
