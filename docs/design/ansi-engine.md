# The ANSI engine

The engine behind the `ansi` format of `render` and behind `show`: a note
drawn for a terminal by the [patine](https://crates.io/crates/patine)
library. The decisions are
[ADR 0082](../adr/0082-ansi-render-format-drawn-by-patine.md) and
[ADR 0083](../adr/0083-show-prints-a-note-drawn-for-the-terminal.md); the
shared command surface and preparation are in [rendering.md](rendering.md),
`show` in [cli.md](cli.md).

## Shape

The engine hands patine the note as written: two `---` fences around the
frontmatter block exactly as the file has it
(`PreparedDocument::frontmatter_source`), then the verbatim body. The
fences are written as plain `---` lines. ntropy accepts a fence followed by
any whitespace, patine only by spaces and tabs, and patine draws the fences
from the marker in either case, so the drawing is the same.

patine lays the source out as styled lines at a width: 80 columns for the
artifact (`DEFAULT_WIDTH`). Running text wraps there; code blocks,
frontmatter and words longer than the width do not. patine is built
without its default `cli` feature, so only the library is compiled.

What patine draws for each construct:

| Construct | Drawing |
| :--- | :--- |
| Frontmatter | the block between its fences, indented and highlighted as YAML |
| Headings | without `#` markers; level one bold, italic and double-underlined, level two bold and underlined |
| Links | the text underlined, the target dimmed after it in parentheses; a note link is a link like any other, so its target is the note's file name |
| Bare URLs and email addresses | underlined; a `www.` host has its `http://` target dimmed after it |
| Images | `[image: alt]` with the source dimmed after it |
| Inline code, code blocks | in the palette's code colour; a code block indented by two columns, a fence with a known language highlighted |
| Block quotes and callouts | behind `│ ` bars; a callout's `[!NOTE]` marker stays text |
| Tables | drawn with box characters |
| Task lists | `☑` and `☐`, a done task dimmed |
| Footnotes | a dimmed `[n]` at each reference, the definitions after the body |
| Thematic breaks | a line across the width |
| Raw HTML | dropped with no warning; the text between inline tags stays |

patine styles with bold, italic, dim, underline, strikethrough and the 8
colours of the ANSI palette, so the terminal's theme picks the actual
colours.

## Escaping

patine writes text verbatim, and CommonMark decodes a numeric character
reference such as `&#x1b;` to the character it names, in running text and in
a link's destination alike. The source therefore cannot be escaped; the
drawn text is. Each span's text passes through one function on its way out:

- a tab becomes the spaces up to the next 8-column stop, counted from the
  start of the line, the indent and quote bars included;
- every character [ADR 0071](../adr/0071-escaping-untrusted-text-for-the-terminal.md)
  escapes (the C0 and C1 controls, DEL, the bidi overrides, embeddings and
  isolates) is written as `char::escape_debug` writes it, so `&#x1b;[2J`
  shows as `\u{1b}[2J`.

A line holding escaped characters is wider than the layout width by the
escapes' length.

## The writer

ntropy writes the styled lines itself rather than through patine's writer,
which takes Markdown and not lines. Each span switches its attributes on,
writes its text and switches them off in reverse order, with the sequences
patine's own writer uses: `1` bold, `2` dim, `3` italic, `4` or `4:2`
underline, `9` strikethrough, `38;5;<n>` the colour, then `39`, `29`, `24`,
`23` and one `22` for bold and dim. No style carries over a line break.

A drawing has one of three forms:

- `Colored`: every style. The artifact uses it, and `show` on a terminal.
- `Styled`: every style but the colours, with patine's dim quote bars
  instead of cyan ones. `show` uses it on a terminal under `NO_COLOR`.
- `Plain`: the text alone, which `show` writes when stdout is not a
  terminal.

Beside the text the engine returns each line's width as patine measures it,
grapheme by grapheme, after escaping. `Drawn::rows` counts each line as the
rows it wraps into at a given terminal width; `show` compares that with the
terminal's rows to decide on the pager.

## Two entry points

`draw` takes a `PreparedDocument`, as every engine does. `draw_source`
takes the frontmatter block and the body alone, which is all the drawing
reads. `show` calls it with the selected note's own fields, so it needs
neither the link index nor the second scan of the vault that preparing a
document takes.

## Testing

- The `Colored` form of the kitchen-sink fixture and of a note with every
  inline style equals `patine::render` of the same source byte for byte,
  pinning the writer against the patine in `Cargo.lock`.
- `Styled` equals `Colored` without its colour sequences.
- Hostile text: raw and decoded control characters, in text, frontmatter and
  link destinations, come out escaped, and no escape character is left
  outside a well-formed style sequence.
- Tabs, widths and row counts, including a halfwidth sound mark that patine
  gives a column of its own.
- CLI tests render `--to ansi` from a plaintext and an encrypted vault and
  compare the artifacts.
