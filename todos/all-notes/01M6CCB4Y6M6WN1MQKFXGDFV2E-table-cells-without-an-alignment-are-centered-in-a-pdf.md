---
title: "Table cells without an alignment are centered in a PDF"
kind: bug
component: render
origin: review
---
# Table cells without an alignment are centered in a PDF

A table column whose delimiter row sets no alignment (`| --- |`) is
left-aligned in HTML but centered in a PDF. Found on 2026-11-01 in the
review of the theme test material, reproduced with ntropy 2.9.0.

## Problem

For

```markdown
| Term | Meaning |
| --- | --- |
| Canary | The first five percent of traffic |
| Everyone | All |
```

the PDF centers every header and body cell ("All" sits in the middle of
the second column). The emitter writes `align: (auto, auto)`, and the
built-in prelude centers the whole table on the page with
`show table: it => align(center, it)` (`src/render/typst/prelude.typ:151`).
The `auto` cells inherit that center alignment. The HTML stylesheet sets
`th, td { text-align: left }` (`src/site/theme/style.css:567-571`).

Related: because the emitter always writes `align:`, a theme's
`set table(align: ...)` has no effect on any column. Emitting the
alignment only for columns that set one (for example through
`table.cell(align:)`) would leave the rest to the theme.

## Suggested fix

Keep the table centered on the page, but left-align the cells of columns
without an alignment, as HTML does. Columns with `:---`, `:---:` or
`---:` keep their alignment.
