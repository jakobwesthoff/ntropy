# 60. Picker accept keys and Ctrl-Y to print the path

Date: 2026-10-10

## Status

Accepted

Amends [ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md),
whose picker ended a selection only with Enter.

## Context

`ntropy search` opens the chosen note in the editor unless `-p`/`--print`
was given ([ADR 0035](0035-generic-print-flag-replaces-no-edit.md)), and that choice
is fixed before the picker opens. Forgetting `-p` when only the path is
wanted happens often, and the only way out was to abort and search again.

## Decision

- A picker caller can give the picker extra accept keys besides Enter: a
  Ctrl letter, a short hint and an action the caller receives together with
  the selected item. Ctrl-C, Ctrl-N, Ctrl-P, Ctrl-U and Ctrl-W stay the
  picker's own and cannot be taken.
- The search picker's Ctrl-Y selects the highlighted note and prints its path
  to stdout as `--print` does, whatever other flags were given. The delete
  and render pickers have no extra keys.
- The stats line shows the accept keys right-aligned at its end, such as
  `^Y path`. The hints show only with at least two free columns before them,
  and they are the first part of the line to go when it is too narrow.
