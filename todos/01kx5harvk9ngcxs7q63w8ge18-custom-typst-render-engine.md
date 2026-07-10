# Custom Typst render engine: remaining work

The custom typst engine is implemented: ntropy converts notes to Typst with
its own `pulldown-cmark` emitter and typesets `pdf` by running the external
`typst` binary, with `typst` as a second output format. The decision is
recorded in ADR 0040, the design in `docs/design/typst-engine.md`, the command
surface in `docs/design/rendering.md`. What follows is the work deliberately
left out of that implementation.

## Theming

The redesigned default theme is implemented (prelude defining `note`,
`callout`, `notelink`, `task`; a4, chips, per-kind callout colors, code
chips/panels, drawn checkboxes, colored links). Remaining theming work:

- **Paper-size configuration** (decided 2026-07-10, not yet built): a
  general render option in the vault config (`.ntropy/config.toml`,
  `[render] paper = "a4"`, default a4) — a serde serialize/deserializable
  **enum** of supported paper formats defined in the renderer (initial
  variants proposed: a4, a5, us-letter, us-legal), with each renderer
  deciding how to honor the setting; the typst engine passes it as the
  typed `paper:` argument the default theme's `note` already accepts.
- **User-provided themes**: the mechanism for a vault to replace the
  embedded prelude with its own. Not yet designed.
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
