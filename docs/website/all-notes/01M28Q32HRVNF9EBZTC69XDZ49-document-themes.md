---
title: Document themes
tags: [docs/themes]
site:
  order: 4
---
A document theme is one Typst file under `.ntropy/themes/typst/` that
gives a vault's PDFs their own look. It redefines what it wants and
inherits the rest. This page is the reference: what a theme may
override, what it receives, what the note's body is made of, fonts,
assets, errors, and the paper size.
[Writing a document theme](01M6CAJG6C3WAZ2MD6W3EGBMKY-writing-a-document-theme.md)
builds one step by step.

## The theme file

A theme named `corporate` is the file
`.ntropy/themes/typst/corporate.typ`, selected with `theme = "corporate"`
in the `[render]` table of `.ntropy/config.toml`; selecting is described
in [Themes](01M6CJK3JX0YS4R2FFHPJTG0JG-themes.md). `ntropy init` does not
create the directory, so make it first:

```bash
mkdir -p .ntropy/themes/typst
```

From then on every `ntropy render` in the vault uses the theme, with
nothing extra on the command line. A configured theme with no file fails
before the vault is scanned, with an error naming the path it looked
for. A theme that does not compile fails with typst's error, described
under [Errors and warnings](#errors-and-warnings). ntropy ships no named
themes of its own beyond the built-in default; themes are the vault's
own files.

## What a theme overrides

This theme puts a logo in the page header and drops the metadata strip,
so internal tags and `status: draft` never reach a customer:

```typst
// .ntropy/themes/typst/corporate.typ
#let note(title: none, frontmatter: (:), paper: "a4", body) = {
  set document(title: title) if title != none

  set page(
    paper: paper,
    margin: (x: 2.2cm, top: 3.4cm, bottom: 2.4cm),
    header: {
      align(right, image("/assets/logo.svg", width: 3.2cm))
      v(-0.4em)
      line(length: 100%, stroke: 0.6pt + rgb("#2dd4bf"))
    },
  )
  set text(size: 11pt)

  if title != none {
    text(size: 1.6em, weight: "bold", title)
    v(0.8em)
  }

  // No frontmatter strip: nothing but the body below the title.
  body
}
```

Four functions are yours to override; a theme defining only `note` keeps
the built-in look for the others:

| Function | Signature | Renders |
| :--- | :--- | :--- |
| `note` | `note(title: none, frontmatter: (:), paper: "a4", body)` | the whole document |
| `callout` | `callout(kind: "note", body)` | a `> [!NOTE]` admonition |
| `notelink` | `notelink(body)` | the text of a note link: the target's title, or the link's text when it carries a heading anchor and has text |
| `task` | `task(done: false)` | a task-list checkbox |

The theme's source is spliced into the emitted document after ntropy's
own definitions and before the document applies `note`. Typst binds a
name to the last definition above its use, which is why a theme's
`note` wins and a function the theme leaves alone keeps the built-in
one. The built-in definitions also include the helpers `fmt-value`,
`is-empty`, and `callout-styles`, which a theme may call; redefining one
of them does not change the built-in `note` or `callout`.

The built-in `note` is more than the title block: it also holds the rules
that style headings, code, quotes, tables, links and rules. A `note`
written from scratch, like the one above, drops all of them, and the body
comes out in typst's plain defaults. Starting from a copy of the built-in
`note` keeps them;
[Writing a document theme](01M6CAJG6C3WAZ2MD6W3EGBMKY-writing-a-document-theme.md#step-1-copy-the-default-and-switch-it-on)
shows how.

A theme needs no `note` of its own for a smaller change. `set` and `show`
rules at the top of the theme file apply to the whole document and keep
the built-in layout:

```typst
// .ntropy/themes/typst/plex.typ
#set text(font: "IBM Plex Sans")
#show heading: set text(fill: rgb("#0f766e"))
```

Where the built-in `note` sets a property itself, its rule comes later
and wins: the text size, the paragraph justification and the page margins
change only in a `note` of your own.

## What `note` receives

- `title`: the note's title as a string.
- `frontmatter`: the note's frontmatter as written, `title` and `tags`
  included, as Typst values. Strings, integers, floats, booleans and
  `null` become Typst strings, integers, floats, booleans and `none`;
  lists become arrays and mappings dictionaries, at any depth. A YAML date
  such as `2026-11-01` stays a string, and a value with a YAML tag
  (`!degrees 90`) becomes the string of its YAML text, tag included.
  `frontmatter.at("author", default: none)` reads a field a note may
  lack. The note's date, which lives in its file name, is not passed.
- `paper`: the paper size, see [Paper size](#paper-size).
- `body`: the converted note.

The body leaves out a level-one heading that opens it and reads exactly
the note's title, as every note made from ntropy's default template does; the
title is `note`'s to draw. A `note` that does not draw `title` therefore
shows no title at all. The PDF outline has no entry for that heading, and
a theme that numbers headings counts the sections of such a note from
`0.1`.

## What the body is made of

The body is plain Typst, so a theme styles it with `set` and `show` rules
on these elements:

| Markdown | Typst element |
| :--- | :--- |
| `#` to `######` headings | `heading`, levels 1 to 6 |
| paragraphs | paragraphs separated by blank lines |
| `- item` | `list` |
| `1. item` | `enum`, starting at the list's first number and counting up from it |
| `- [ ] task` | a `list` item that starts with `task(done: ...)` and a space; the list's bullet stays unless the theme changes `list` |
| `> quote` | `quote(block: true)` |
| `> [!NOTE]` and the other four kinds | `callout(kind: "note")`, the kind in lower case |
| pipe table | `table` with a `table.header`; each cell of a column with a Markdown alignment is a `table.cell(align: ...)`, and no `align:` is passed |
| fenced or indented code | `raw` block, with the fence's language |
| `` `code` `` | `raw` inline |
| `*em*`, `**strong**`, `~~gone~~` | `emph`, `strong`, `strike` |
| `[text](url)`, bare URLs | `link` |
| a note link | `link` to `<slug>.pdf` around `notelink`, or `link` to the heading's label when the link names a heading of the note itself |
| `![alt](path)` with a local path | `image` with a path from the vault root and `alt:` set to the alt text, left out when the alt text is blank |
| `[^label]` | `footnote` |
| `---` | `line(length: 100%)` |
| a hard line break | `linebreak()` |

A column with a Markdown alignment keeps it, as every one of its cells
sets `align` itself. A column without one follows
`set table(align: ...)`; the built-in look sets `start`. A quote with any
other marker, such as `> [!FOO]`, stays a `quote`, so `callout` only ever
sees the five kinds. Raw HTML is dropped, with a warning naming it.

The built-in look also measures every column and sets the column widths
itself, so cells do not run into each other. Widths come from the text of
the cells, so an image in a cell counts as no width.

- When the widest word of each column does not fit the page, the table
  is set in smaller text, down to 60 percent of the size.
- A table that would need smaller text stays at 60 percent, runs past
  the right edge of the page and loses the columns beyond it. A PDF
  render warns about it, and `--strict` turns the warning into a
  failure.
- `--to typst` gives no such warning, since the check runs typst on the
  document after the PDF compile. If that check cannot run, the render
  warns that it could not check the tables and still finishes.
- A theme that wraps tables in a padded block takes that padding out of
  the columns of a squeezed table.

Every footnote is drawn once, at its first reference in the text, and
every later reference shows the same number. Headings carry labels that
start with `ntropy-h-`, and footnotes carry labels that start with
`ntropy-fn-`. Do not define labels with these prefixes. A theme must not
draw the body twice, nor a block that holds a heading or a footnote with a
repeated reference: Typst rejects the duplicated label as soon as a link
names that heading or a later reference names that footnote, and the
compile fails. An `outline()` of the headings compiles, unless a heading
holds a footnote the note references again. A `show footnote` rule runs
for the repeated references as well, and `it.body` is a label there, so a
rule that prints the body must check `type(it.body) == label`.

## Fonts

The built-in look sets its text in the fonts typst brings along:
Libertinus Serif, New Computer Modern and DejaVu Sans Mono. The tick in a
done task box is drawn as a shape, so it needs no font. Libertinus Serif
covers Latin, Greek, Cyrillic and Hebrew. For a script it does not cover,
such as Arabic or Japanese, typst takes the glyphs from a font it finds on
the machine, so the PDF looks different from machine to machine. Without
such a font typst falls back to DejaVu Sans Mono for Arabic, which has a
monospace look, and Japanese characters become empty boxes, with no warning
from ntropy, also under `--strict`.

A theme makes such a script look the same everywhere by naming a font for
it, after the one it already uses:

```typst
#set text(font: ("Libertinus Serif", "Noto Serif CJK JP"))
```

The font must be installed or found through `TYPST_FONT_PATHS`, as below.

A theme can use every font typst finds: the fonts installed on the system,
and the fonts in the directories named by the `TYPST_FONT_PATHS`
environment variable. ntropy passes typst no font directory of its own,
so a font file in the vault is found only when `TYPST_FONT_PATHS` points
at it. A relative path there is resolved against the directory you run
ntropy in, so an absolute one is safer:

```bash
export TYPST_FONT_PATHS="$(ntropy info -p)/assets/fonts"
```

`typst fonts` lists the names typst can use, and
`typst fonts --font-path <dir>` includes a directory.

A font name typst does not find falls back to the default font. The
render still succeeds, with typst's warning `unknown font family: <name>`
as one of its warnings, and `--strict` fails on it. `pdffonts` from
poppler lists the fonts a PDF embeds.

## Assets and the vault root

The logo above lives at `<vault>/assets/logo.svg`, outside `all-notes/`,
which holds notes and nothing else. typst compiles the document as if it
sat at the vault root, the root it resolves paths against. A path in
a theme is therefore read from the vault root whether it starts with `/`
or not: `/assets/logo.svg` and `assets/logo.svg` name the same file.

A theme can split its code into more files and import them by such a
path, `#import "/.ntropy/themes/typst/colors.typ": accent`. An imported
file is a module of its own: it sees neither ntropy's definitions such as
`fmt-value` nor the names of the theme file. Pass it what it needs as
arguments, or keep the code that uses them in the theme file.

## Errors and warnings

A theme that does not compile fails the render with typst's error, which
names where the problem is. `<stdin>` with a line number means a line of
the emitted document, the file `--to typst` writes: the theme starts after
ntropy's own definitions, so that line is further down than in the theme
file. An error in a file the theme imports names that file and its own
line.

A compile that succeeds can still print warnings, such as an unknown font
family. A PDF render shows each of them as one warning of ntropy, and
`--strict` fails on it. The line holds typst's message, the source line it
points at in backticks and typst's hints, if it has any:

```
warning: unknown font family: no-such-font (in `set text(font: "no-such-font")`)
```

The line number is left out, as it counts lines of the emitted
document. typst prints nothing for a character no font covers, so that
case stays silent.

The `--to typst` format runs no compiler and shows no typst warnings.
Compiling its artifact by hand prints them, and shows the emitted line an
error names. Since a theme's assets are addressed from the vault root,
the compile takes the vault as its root. typst refuses a document file
outside that root, so hand the artifact over on standard input, as
ntropy does:

```bash
typst compile --root <vault> - note.pdf < note.typ
```

To see a theme draw the parts of a note in one document, render the
theme test note, described in Themes.

## Paper size

Rendering defaults to a4. A `[render]` section in the vault's
`.ntropy/config.toml` picks a different format:

```toml
[render]
paper = "us-letter"
```

Supported values: `a3`, `a4`, `a5`, `iso-b5`, `jis-b5`, `us-letter`,
`us-legal`, `us-tabloid`, `us-executive`, `us-oficio`. An unknown value
is a config error naming the bad name, reported before anything renders.
The value reaches the theme as the `paper` argument of `note`, so a theme
whose `set page` does not pass it on ignores the setting.
