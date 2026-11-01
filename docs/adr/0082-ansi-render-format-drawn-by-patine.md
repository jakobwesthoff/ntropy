# 82. ANSI render format drawn by patine

Date: 2026-11-01

## Status

Accepted

Adds a format to the registry of
[ADR 0038](0038-pluggable-rendering-engine-with-pandoc-and-typst.md), whose
engine abstraction lives on under
[ADR 0040](0040-custom-typst-engine-with-own-markdown-emitter.md). Amends
[ADR 0071](0071-escaping-untrusted-text-for-the-terminal.md): a drawn note's
text is escaped too. `docs/design/ansi-engine.md` carries the model.

Amended by [ADR 0084](0084-ansi-drawing-follows-patine-2-2.md).

## Context

ntropy renders a note to PDF, Typst and HTML. Nothing shows a note formatted
in a terminal: `search -P` prints its source. patine lays Markdown out as
ANSI-styled lines, as a library and as a command.

## Decision

A format `ansi`, produced by an engine `ansi`, with the extension `ansi`.

The engine hands patine the note as written: its frontmatter block between
two `---` fences, then its body. patine lays it out at a width, 80 columns
for the artifact. It shows the frontmatter as a highlighted block, a link as
its text with its target dimmed after it, a note link included, and an image
as `[image: alt] (url)`. It drops raw HTML, and the engine reports nothing
for it. patine styles with the 8 colours of the ANSI palette, so the
terminal's theme picks the actual colours. The library is built with
`default-features = false`, which leaves out patine's own command line.

ntropy writes the escape sequences itself from patine's styled lines. Before
a span is written, a tab becomes the spaces to the next 8-column stop, and
every other character ADR 0071 escapes is escaped the same way. patine
writes text verbatim, and a numeric character reference such as `&#x1b;`
decodes to the character it names, so escaping the source would not keep a
control character out. Each span switches its attributes on and off as
patine's own writer does.

The lines can be written in three forms: with every style, which the
artifact uses; with every style but the colours; and as text alone.

The `ansi` format takes no theme. `--theme` with it is an error, and a
configured theme is not loaded.

## Consequences

- patine brings comrak, syntect and two-face: the release binary grows from
  8,057,472 to 10,106,976 bytes on macOS (Apple Silicon).
- A line holding escaped characters is wider than the layout width by the
  escapes' length.
- patine's `TextStyle` is `#[non_exhaustive]`: a style a later patine adds
  is dropped by ntropy's writer until the writer learns it, and the test
  that compares the writer with patine's own catches that only when its
  fixtures use the new style.
