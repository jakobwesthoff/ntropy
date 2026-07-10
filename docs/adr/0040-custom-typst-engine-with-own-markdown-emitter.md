# 40. Custom typst engine with own Markdown emitter

Date: 2026-07-10

## Status

Accepted

Supersedes the engine choice of
[ADR 0038](0038-pluggable-rendering-engine-with-pandoc-and-typst.md)
while keeping its engine abstraction, registry, and capability model.
`docs/design/typst-engine.md` carries the full model; research distilled
from a prior converter implementation lives in
`docs/research/markdown-to-typst-conversion.md`.

## Context

The pandoc engine of ADR 0038 was a stopgap. Of the two tools it
requires, only typst typesets; pandoc contributes solely the
Markdown-to-Typst conversion, and that indirection blocks the roadmap:
styling must pass through pandoc's template layer instead of a Typst
template ntropy owns, metadata is spliced into typst code by the stock
template (the `keywords` slot is unusable, tags ride in `subtitle`), and
note links can only be flattened to what survives a Markdown round-trip.

## Decision

A new engine, registered as `typst`, converts the note to Typst markup
with ntropy's own emitter built on the `pulldown-cmark` parser and
produces two formats: `typst` (the emitted document as the artifact) and
`pdf` (that identical document compiled by the external `typst` binary,
fed on stdin with the note's directory as working directory, so relative
asset paths resolve as if the document sat next to the note and the
vault stays untouched). `Invocation` grows a stdin payload and a
working-directory field for this.

The supported input surface is what GitHub renders, with decided
exceptions and degradations recorded in the design document; math is not
supported for now (deferred to
`todos/01kx5n2ww5526gtfmhga2b8xe4-typst-engine-math-support-via-mitex.md`).

The emitted Typst is an intermediate representation: readability is a
non-goal, and the emitter always prefers mechanically foolproof forms —
unconditional escaping of every markup-active character, function calls
for span styling, metadata as typed Typst values — over idiomatic
hand-written Typst. The escape rules derive from Typst's own lexer and
are verified by round-trip tests through the `typst-syntax` parser as a
dev-dependency, keeping ADR 0038's rule that tests execute no external
tools. The document skeleton applies an inlined, engine-owned prelude
via `#show: note.with(...)`, which receives the complete frontmatter as
typed values and is the designated theming hook.

Once the new engine delivers complete, working PDF conversion, it
becomes the default for `pdf`, with the stated intent to then remove
the pandoc engine entirely.

## Consequences

- `pulldown-cmark` joins the runtime dependency tree; `typst-syntax`
  joins dev-dependencies. The `pdf` format's only external tool is
  `typst`; `render --to typst` needs no external tool at all.
- Rendering correctness (escaping, element mapping) becomes ntropy's
  own responsibility, pinned by an in-repo verification corpus instead
  of pandoc's behavior.
- Assets referenced above the note's directory fail the pdf compile
  with typst's project-root sandbox error.
- `playground/typst/` holds the machine-verified escaping and writer
  code developed during design, to be lifted into `src/render/`.
