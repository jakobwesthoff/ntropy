# 44. Cross-document links between rendered notes

Date: 2026-09-07

## Status

Accepted

Extends [ADR 0028](0028-note-to-note-links-as-standard-markdown-links.md) into
the rendered artifact, changing the emission the typst engine of
[ADR 0040](0040-custom-typst-engine-with-own-markdown-emitter.md) performs for a
resolved note link. It ties that emission to the default artifact name of
[ADR 0037](0037-render-command-surface.md).

## Context

A note-to-note link reached the artifact as `#notelink[Title]`: the target's
current title, styled by a prelude function, carrying no link annotation. A
reader of a rendered note saw that a cross-reference existed but had no way to
follow it.

`render` produces one note per invocation (ADR 0037), so a set of notes becomes
a set of separate files, each named `./<slug>.<ext>` from the slug component of
the note's filename unless `-o` says otherwise.

## Decision

A resolved note link emits `#link("<target-slug>.pdf")[#notelink[Title]]`. The
`notelink` wrapper is unchanged, so a theme still styles note links distinctly;
the surrounding `#link` adds the annotation.

- The target is the target note's **current filename slug** plus `.pdf`, with no
  directory part. It is read from the vault at render time, not from what the
  link in the body spells out, so a stale slug in the Markdown does not reach
  the artifact.
- That name is exactly what a default `ntropy render` of the target note
  produces (ADR 0037). Rendering a set of notes into one directory therefore
  yields artifacts that reference each other by name.
- The extension is always `pdf`, never the extension of the format being
  produced. Both formats emit identical document bytes, the `typst` artifact
  being the source of a PDF.
- A dangling note link is unaffected: the wrapper is dropped and the display
  text's markup re-emitted.
- The naming convention is user-facing documentation, since the link only finds
  its target when the target was rendered under its default name.

### Rejected alternatives

- **A PDF-native cross-document action (`/GoToR`, `/Launch`).** Typst's link
  destination accepts a URL string, a label, a location, or a `page`/`x`/`y`
  dictionary; none of them selects a different PDF action type.
- **One document holding several notes, with internal links.** It requires a
  `render` that takes a set of notes, which the one-note-per-invocation surface
  of ADR 0037 does not provide.

## Consequences

- A resolved note link in a rendered PDF carries a `/Link` annotation whose
  action is a `/URI` holding the relative name `<slug>.pdf`.
- Whether clicking it opens the sibling file is the viewer's behavior, and is
  not verified across viewers.
- The link finds its target only when both notes were rendered into the same
  directory under their default names. An `-o` path elsewhere, or a target that
  was never rendered, leaves the annotation pointing at a file that is not
  there.
- Two notes whose filenames carry the same slug render to the same artifact
  name, so links to either resolve to whichever was written last.
- The `typst`-format artifact carries the same `.pdf` targets as the PDF does.
