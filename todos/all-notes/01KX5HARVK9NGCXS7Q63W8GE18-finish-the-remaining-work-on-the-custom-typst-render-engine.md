---
title: "Finish the remaining work on the custom Typst render engine"
kind: feature
component: render
origin: discussion
---
# Finish the remaining work on the custom Typst render engine

The custom Typst engine is implemented: ntropy converts notes to Typst with
its own `pulldown-cmark` emitter and typesets `pdf` by running the external
`typst` binary, with `typst` as a second output format. The decision is
recorded in ADR 0040, the design in `docs/design/typst-engine.md`, and the
command surface in `docs/design/rendering.md`. This todo collects the work
deliberately left out of that implementation. It is one todo for now; the
areas below are independent of each other.

## Theming

The redesigned default theme is implemented (prelude defining `note`,
`callout`, `notelink`, `task`; a4, chips, per-kind callout colors, code
chips and panels, drawn checkboxes, colored links). Remaining theming work:

- User-provided themes: the mechanism for a vault to replace the embedded
  prelude with its own. Not yet designed.
- Smart quotes. The emitter escapes `'` and `"` unconditionally, so they
  render as straight quotes in the PDF and a theme cannot re-smarten them.
  Typographic quotes need a narrow emitter change: stop escaping quotes.
  This is the one place theming reaches back into the emitter.

## Decisions

- 2026-07-10, user: smart quotes are deferred again during basic theming.

## Assets above the note's directory

The pdf pipeline feeds the document on stdin with the note's directory as the
working directory, which is also typst's file-access sandbox. Assets
referenced with `../` above the note's directory therefore fail the compile
with typst's explicit "would escape the project root" error (design doc,
"Asset paths"). A contained extension is an opt-in variant that passes a
`--root` and rewrites paths, widening the sandbox while keeping the
note-relative resolution that the `typst` format's identical-bytes contract
depends on.

## Math

Math is deferred to its own todo, which holds the decision trail and the
build path through the mitex LaTeX-to-Typst translator. The interim behavior
ships: `ENABLE_MATH` is off, `$` renders as literal escaped text, and `math`
fences render as plain code blocks, with no warning.
