# 60. Additional picker actions on accept keys

Date: 2026-10-10

## Status

Accepted

Amends [ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md),
whose picker ended a selection only with Enter.

## Context

What a command does with the note chosen in the picker is fixed before the
picker opens: `ntropy search` opens it in the editor unless `-p`/`--print`
was given ([ADR 0035](0035-generic-print-flag-replaces-no-edit.md)). Wanting
a different action than the flags chose meant aborting and running the
command again. The first case is forgetting `-p` when only the path is
wanted, which happens often.

## Decision

- Besides Enter, a picker can end a selection with additional accept keys,
  each one standing for a different action on the selected item. A command
  that opens a picker declares the keys it supports: a Ctrl letter, a short
  hint and an action it receives together with the selected item. Ctrl-C,
  Ctrl-N, Ctrl-P, Ctrl-U and Ctrl-W stay the picker's own and cannot be
  taken.
- The stats line shows the declared keys right-aligned at its end, such as
  `^Y path`. The hints show only with at least two free columns before them,
  and they are the first part of the line to go when it is too narrow.
- The first such key is Ctrl-Y in the search picker: it selects the
  highlighted note and prints its path to stdout as `--print` does, whatever
  other flags were given. The delete and render pickers declare no keys.
