---
title: "Render typographic quotes in Typst output"
kind: feature
component: render
origin: discussion
---
# Render typographic quotes in Typst output

The Typst emitter escapes `'` and `"` unconditionally, so they render as
straight quotes in the PDF and a theme cannot re-smarten them. Typographic
quotes need a narrow emitter change: stop escaping quotes. This is the one
place theming reaches back into the emitter. The engine is described in
ADR 0040 and `docs/design/typst-engine.md`, and themes in ADR 0045.

## Decisions

- 2026-07-10, user: smart quotes are deferred again during basic theming.
- 2026-10-30, user: verify the `../` behaviour with a render, correct the two stale texts, then the todo shrinks to smart quotes only.
