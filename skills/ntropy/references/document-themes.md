---
name: document-themes
description: >-
  Write or adapt a document theme for `render` to pdf and typst: where the
  Typst file lives and how it is selected, how it is spliced into the
  emitted document, what `note` receives, the functions it may override,
  the body's elements as show-rule hooks, paper, assets, imports, fonts and
  TYPST_FONT_PATHS, and how to see typst's warnings and errors.
metadata:
  tags: theme, typst, pdf, render, fonts, paper, show rules
---

# Document themes

A document theme is one Typst file, `<vault>/.ntropy/themes/typst/<name>.typ`,
selected by `[render] theme = "<name>"` in `.ntropy/config.toml` or by
`--theme <name>` for one render. `--theme default` (or `theme = "default"`)
is the built-in look. It applies to `render` with `--to pdf` (the default)
and `--to typst`; `html` uses a site theme, `ansi` none. `ntropy init` does
not create `.ntropy/themes/typst/`. A missing theme fails before the vault
is scanned; a theme that does not compile fails the render. Neither falls
back to the built-in look.

## How a theme is applied

`render --to typst` writes the document that `--to pdf` compiles. It is, in
order: ntropy's own definitions (the prelude), the line
`// ---- vault theme ----` and the theme file verbatim, then
`#show: note.with(title: ..., frontmatter: ..., paper: ...,)` and the
converted body. Typst binds a name to the last definition above its use,
so a theme's `note`, `callout`, `notelink` or `task` replaces the built-in
one and the others stay.

| Function | Signature | Called for |
| :--- | :--- | :--- |
| `note` | `note(title: none, frontmatter: (:), paper: "a4", body)` | the whole document, through `#show` |
| `callout` | `callout(kind: "note", body)` | each `> [!NOTE]`-style alert; `kind` is `note`, `tip`, `important`, `warning` or `caution` |
| `notelink` | `notelink(body)` | the text of a note link, inside `#link("<slug>.pdf")`: the target's title, or the link's text when it carries a heading anchor and has text. A link to the note itself with an anchor goes to the heading's `<ntropy-h-N>` label instead, to the top of the first page for `#top`, and nowhere when the anchor names no heading |
| `task` | `task(done: false)` | a task-list checkbox |

The prelude's helpers `fmt-value` (any frontmatter value as text),
`is-empty` and `callout-styles` can be called from the theme. Redefining
one does not change the built-in `note` or `callout`.

Two ways to write a theme:

- Top-level `#set` and `#show` rules in the theme file apply to the whole
  document and keep the built-in `note`, for example
  `#set text(font: "IBM Plex Sans")`. Where the built-in `note` sets a
  property itself (text size, paragraph justification, page margins and
  paper), its rule wins, so those change only in a `note` of the theme's
  own.
- A `note` of the theme's own. The built-in `note` also holds the rules
  that style headings, inline and block code, quotes, tables, links and
  rules; a `note` written from scratch has none of them. To start from the
  built-in one, copy it out of a render:

  ```bash
  ntropy render -n --to typst --theme default --force -o /tmp/default.typ <id>
  sed -n '/^#let note(/,/^}/p' /tmp/default.typ > .ntropy/themes/typst/<name>.typ
  ```

  `--theme default` matters in a vault that already has a theme; without
  it the copy holds two `note` definitions.

## What `note` receives

- `title`: the note's title, a string.
- `frontmatter`: the frontmatter as written, `title` and `tags` included,
  as Typst values: strings, integers, floats, booleans, `none` for
  `null`, arrays, dictionaries, at any depth. A YAML date stays a string; a
  tagged value (`!degrees 90`) is the string of its YAML text. Read a field
  a note may lack with `frontmatter.at("author", default: none)`. The
  note's creation date (in its file name) is not passed.
- `paper`: the configured paper name, `a4` unless `[render] paper` says
  otherwise (`a3`, `a4`, `a5`, `iso-b5`, `jis-b5`, `us-letter`, `us-legal`,
  `us-tabloid`, `us-executive`, `us-oficio`). A theme whose `set page` does
  not pass `paper: paper` ignores the setting.
- `body`: the converted note. A level-one heading that opens the body and
  reads exactly the title is left out, so `note` has to draw `title` or the
  PDF shows none; a theme numbering headings counts such a note's sections
  from `0.1`.

## The body's elements

Show and set rules inside `note` style these:

| Markdown | Typst |
| :--- | :--- |
| `#` to `######` | `heading`, levels 1 to 6 |
| `- item` / `1. item` | `list` / `enum` (from the first item's number, counting up) |
| `- [ ] task` | `list` item starting with `task(done: ...)` and a space; the bullet stays unless `list` is changed |
| `> quote` | `quote(block: true)` |
| `> [!NOTE]` and the other four kinds | `callout(kind: ...)`; any other `[!X]` stays a `quote` |
| pipe table | `table(columns:, table.header(...), ...)`; each cell of a column with a Markdown alignment is `table.cell(align: ..)` and no `align:` is passed, so a column without one follows `set table(align: ..)`, which the built-in look sets to `start` |
| code block / `` `code` `` | `raw` block (fence language) / `raw` inline |
| emphasis, strong, strikethrough | `emph`, `strong`, `strike` |
| links, bare URLs, `<mail@host>` | `link` |
| note link | `link("<slug>.pdf")` around `notelink`, or `link(<ntropy-h-N>)` when the link names a heading of the note itself |
| local image | `image("<path from the vault root>", alt: "<alt text>")`, without `alt:` when the alt text is blank |
| remote image | `link` to the URL, with a warning |
| `[^label]` | `footnote` |
| `---` | `line(length: 100%)` |
| hard line break | `linebreak()` |
| raw HTML | dropped, with a warning |

The built-in look measures the columns of a table and sets their widths. When
the widest words do not fit the page it sets the table in smaller text, down
to 60 percent; a table that needs less than that stays at 60 percent, runs
past the right edge of the page and loses the columns beyond it, and a PDF
render warns (`--strict` fails on it). Images in cells count as no width. A
theme that wraps tables in a padded block takes that padding out of the
columns of a squeezed table.

## Assets and imports

typst runs with `--root <vault>` and reads the document on stdin, which
places it at the vault root. Every path in a theme is read from the vault
root, with or without a leading `/`: `image("/assets/logo.svg")` is
`<vault>/assets/logo.svg`.

`#import "/.ntropy/themes/typst/colors.typ": accent` works. An imported
file is a module of its own and sees neither the prelude's names
(`fmt-value`, ...) nor the theme file's; pass values in as arguments.

## Fonts

The built-in look uses typst's bundled fonts (Libertinus Serif, New
Computer Modern, DejaVu Sans Mono). Libertinus Serif covers Latin, Greek,
Cyrillic and Hebrew. A script it does not cover (Arabic or Japanese, say)
comes from a font on the machine, so the PDF differs per machine. Without
one, typst falls back to DejaVu Sans Mono for Arabic and draws Japanese as
empty boxes, with no warning. A
theme names a font for it: `set text(font: ("Libertinus Serif", "<family>"))`.
ntropy passes no
`--font-path`, so a theme can use system fonts and fonts in the
directories of `TYPST_FONT_PATHS`. Font files in the vault are found only
through that variable; a relative value resolves against the directory
ntropy runs in, so set it absolute:
`TYPST_FONT_PATHS="$(ntropy info -p)/assets/fonts"`. `typst fonts`
(`--font-path <dir>` to add one) lists usable family names.

An unknown font name falls back to the default font and typst's warning
`unknown font family: <name>` reaches the user as a render warning, which
`--strict` fails on. `pdffonts <pdf>` lists the fonts a PDF embeds.

## Warnings and errors

A PDF render shows each warning of a compile that succeeds as one ntropy
warning: typst's message, then the source line it points at in backticks,
then its hints; `--strict` fails on it. A character no font covers is
silent, as typst prints nothing for it. `--to typst` runs no compiler. To see
typst's own output, compile the typst artifact by hand, on stdin (typst
refuses a main file outside `--root`):

```bash
ntropy render -n --to typst --force -o /tmp/check.typ <id>
typst compile --root "$(ntropy info -p)" - /tmp/check.pdf < /tmp/check.typ
```

An error cited as `<stdin>:<line>` is a line of that emitted document, not
of the theme file: the theme starts below the prelude. An error in an
imported file names that file and its own line.

A test note holding what a note can contain that a PDF draws comes as a
download with the note it links to and its two images, unpacked into the
vault root:

```bash
curl -fsSL https://ntropy.westhoffswelt.de/downloads/theme-test-note.tar.gz | tar xz -C "$(ntropy info -p)"
ntropy render -n --force -o /tmp/theme-test-note.pdf 'title:"Theme test note"'
```

It adds two notes tagged `theme-test` and the folder `assets/theme-test/`;
in an encrypted vault, `ntropy reconcile` afterwards encrypts the notes.
Remove them with `ntropy delete -n -f 01M6CCEVBP701HH0VPBT6ZNAYZ`,
`ntropy delete -n -f 01M6CCEVCDRQ5504MDH503KQT2` and
`rm -r assets/theme-test`.

Every GitHub release attaches the same archive, packed from the release's
commit: `https://github.com/jakobwesthoff/ntropy/releases/download/<tag>/theme-test-note.tar.gz`
matches that release's binary.
