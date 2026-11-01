---
title: "A link to a heading of the same note goes nowhere in a PDF"
kind: bug
component: render
origin: implementation
---
# A link to a heading of the same note goes nowhere in a PDF

A Markdown link to a heading of the same note, such as
`[back to the lists](#lists)`, jumps to the heading in HTML and on the site,
but in a PDF it is a dead link. Found on 2026-11-01 while building the theme
test note, reproduced with ntropy 2.9.0.

## Problem

For the body

```markdown
## Lists

Text.

## Other

[back to the lists](#lists) and [missing](#nope).
```

`ntropy render --to typst --strict` writes

```typst
#link("#lists")[back to the lists] and #link("#nope")[missing]\.
```

and exits 0 without a warning. Typst takes `"#lists"` as a URL, so the PDF
holds a link a viewer cannot follow. A fragment that names no heading gets
no warning in either format.

## Suggested fix

Resolve an anchor-only target against the note's heading ids, the same ids
the HTML output gives its headings (ntropy 2.8.0), and link the heading
inside the PDF through a Typst label on it. A fragment that names no heading
renders as plain text with a warning, so `--strict` fails. The heading label
needs a reserved prefix, like the footnote labels' `ntropy-fn-`, and the
document-themes note should list it. The design doc's mapping table
(`docs/design/typst-engine.md`) gains the row.
