---
title: "A self-referencing footnote leaks a placeholder into the PDF"
kind: bug
component: render
origin: review
---
# A self-referencing footnote leaks a placeholder into the PDF

A footnote whose definition references itself puts ntropy's internal
placeholder into the Typst output and the PDF, and the render still exits
0. For

```markdown
A[^a] and again[^a].

[^a]: B[^a]
```

`ntropy render --to typst` (v2.9.0) writes

```
A#footnote[B#footnote[B ntropy-footnote-<ULID> a ntropy-footnote-<ULID> ]] and again#footnote[...]
```

so the PDF prints "ntropy-footnote-<ULID>" as text. `--to html` shows no
placeholder. Found on 2026-11-01 in the review of the document theme guide
and reproduced by the lead with a scratch vault.

## Problem

Typst inlines a footnote's content at each reference. `Walker::finish` in
`src/render/markdown/mod.rs` (around lines 640 to 679) substitutes the
placeholders in a bounded number of passes so a cycle terminates, and
whatever placeholder is left after the last pass stays in the output. The
same inlining prints a footnote again, with a new number, at every
reference to it.

## Suggested fix

Give each footnote a Typst label at its first reference
(`#footnote[...] <fn-a>`) and render every later reference, including one
inside its own or another footnote's content, as `#footnote(<fn-a>)`, which
Typst shows with the same number. That ends the recursion and also prints a
footnote once however often it is referenced. Whatever the approach, no
placeholder may reach any output; a leftover one should be a bug the tests
catch.
