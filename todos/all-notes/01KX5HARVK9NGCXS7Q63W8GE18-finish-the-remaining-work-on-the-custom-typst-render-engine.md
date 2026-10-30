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
chips and panels, drawn checkboxes, colored links), and so are vault render
themes (ADR 0045). Remaining theming work:

- Smart quotes. The emitter escapes `'` and `"` unconditionally, so they
  render as straight quotes in the PDF and a theme cannot re-smarten them.
  Typographic quotes need a narrow emitter change: stop escaping quotes.
  This is the one place theming reaches back into the emitter.

## Decisions

- 2026-07-10, user: smart quotes are deferred again during basic theming.

## Asset paths in the design doc

Assets above the note's directory work since ADR 0045: the pdf pipeline
passes `--root <vault>` on every render (`src/render/typst/engine.rs`), and
the emitter rewrites local image paths to root-absolute ones under the
note's directory, so `../assets/logo.svg` reaches a shared vault asset. Only
a path that climbs out of the vault fails (checked in the code, 2026-10-30;
not run against typst). Two texts still describe the older behaviour and
contradict that: the closing paragraph of "Asset paths" in
`docs/design/typst-engine.md` ("assets referenced with `../` above the
note's directory are unsupported") and the comment above the `Format::Pdf`
arm in `engine.rs` ("relative asset paths in the document resolve against
the note"). Correct both, after confirming the behaviour with a render.

## Math

Math is deferred to its own todo, which holds the decision trail and the
build path through the mitex LaTeX-to-Typst translator. The interim behavior
ships: `ENABLE_MATH` is off, `$` renders as literal escaped text, and `math`
fences render as plain code blocks, with no warning.
