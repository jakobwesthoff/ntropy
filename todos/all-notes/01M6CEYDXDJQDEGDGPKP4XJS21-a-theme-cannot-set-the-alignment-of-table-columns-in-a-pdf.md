---
title: "A theme cannot set the alignment of table columns in a PDF"
kind: feature
component: render
origin: discussion
---
# A theme cannot set the alignment of table columns in a PDF

A document theme's `set table(align: ...)` has no effect, because ntropy
writes an `align:` argument for every table, with `auto` for each column
the Markdown leaves unaligned. The docs state this as a limit today.

## Problem

`src/render/typst/emitter.rs`, `render_table` (around line 471 on
f0643bd), always writes `align: (...)`. The prelude then decides the
look of unaligned columns (centered today; left-aligned on branch
`pdf-fonts-dir` through `show table.cell: set align(start)`, which a
theme cannot undo with a `set` rule either).

## Decision

The user decided on 2026-11-01 (AskUserQuestion in the lead session of
the todo run): a theme may set the alignment of columns that have no
alignment in the Markdown. ntropy fixes only the columns written with
`:---`, `:---:` or `---:`; for the rest the theme decides, and the
default look stays left-aligned.

## Suggested fix

Emit alignment only for explicitly aligned columns (for example per cell
or through a function of the column index that falls back to the
inherited value), move the left-aligned default into a `set` rule a
theme can override, and prove both with real-typst tests: an unaligned
column follows a theme's `set table(align: right)`, an aligned one does
not. Update `document-themes`, `skills/ntropy/references/document-themes.md`
and `docs/design/typst-engine.md`, which state the limit today.

## Relations

- Blocked by: branch `pdf-fonts-dir` (the left-aligned default and the
  wide-table rule in the prelude) landing on main.
