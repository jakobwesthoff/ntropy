# Custom Typst render engine: remaining work

The custom typst engine is implemented: ntropy converts notes to Typst with
its own `pulldown-cmark` emitter and typesets `pdf` by running the external
`typst` binary, with `typst` as a second output format. The decision is
recorded in ADR 0040, the design in `docs/design/typst-engine.md`, the command
surface in `docs/design/rendering.md`. What follows is the work deliberately
left out of that implementation.

## Theming

Theming is the next step (user, 2026-07-10): a better default theme and
user-provided themes. The emitted document already carries the designated hook:
an inlined **prelude** defining the `note` and `callout` functions, applied via
`#show: note.with(title:, frontmatter:)`. A theme is a different prelude
defining those two functions; the converted body never changes. Presentation
decisions belong here, not in the emitter: which frontmatter fields to feature
or hide, per-kind callout colors and symbols, and typography.

- **Basic theming (first step, user, 2026-07-10)**: a nicer default look via
  a redesigned default prelude — a4 page setup, typography tuning on the
  bundled fonts (only Libertinus Serif, New Computer Modern, and DejaVu Sans
  Mono ship with typst; system fonts are not portable), subdued metadata
  block, styled code blocks and inline-code chips, per-kind callout colors,
  lighter tables, quieter quotes/rules, colored links. Includes the
  `#notelink` hook (decided): the emitter emits resolved note links through a
  prelude-defined `notelink` function (default look: the emphasized title)
  so themes can style them distinctly. Decisions on configuration (user,
  2026-07-10): paper size is a general render option in the vault config
  (`.ntropy/config.toml`, `[render] paper = "a4"`, default a4) — a serde
  serialize/deserializable **enum** of supported paper formats defined in the
  renderer (initial variants proposed: a4, a5, us-letter, us-legal), with
  each renderer deciding how to honor the setting; the typst engine passes it
  as a typed `paper:` argument into `note.with(...)`, and the default theme
  applies a4 via `set page`. The default theme skips frontmatter fields with
  empty values (empty string/array/mapping, null) — the template still
  receives them; skipping is pure presentation (user, 2026-07-10).
- **Smart quotes revisit.** Deferred again during basic theming (user,
  2026-07-10). The emitter escapes `'` and `"` unconditionally, so they
  render as straight quotes in the PDF and a theme cannot re-smarten them.
  Typographic quotes would need a narrow emitter change: stop escaping
  quotes. This is the one place theming reaches back into the emitter.

## Assets above the note's directory (`--root` extension)

The pdf pipeline feeds the document on stdin with the note's directory as the
working directory, which is also typst's file-access sandbox. Assets
referenced with `../` above the note's directory therefore fail the compile
with typst's explicit "would escape the project root" error (design doc, "Asset
paths"). A contained extension is an opt-in `--root`-plus-path-rewriting
variant that widens the sandbox while keeping the note-relative resolution the
`typst` format's identical-bytes contract depends on.

## Math

Math is deferred with its own decision trail and build path (mitex
LaTeX-to-Typst translator) in
`01kx5n2ww5526gtfmhga2b8xe4-typst-engine-math-support-via-mitex.md`. The
interim behavior ships: `ENABLE_MATH` is off, `$` renders as literal escaped
text, and `math` fences render as plain code blocks, with no warning.
