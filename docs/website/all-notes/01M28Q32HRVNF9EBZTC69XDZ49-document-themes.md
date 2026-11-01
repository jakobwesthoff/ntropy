---
title: Document themes
tags: [docs/publish]
site:
  order: 3
---
A vault can render its PDFs in its own livery. A document theme is one
Typst file under `.ntropy/themes/typst/` that redefines what it wants and
inherits the rest. This page covers how to set one up, what a theme may
override, what it receives, what the note's body is made of, fonts, how it
reaches assets in the vault, and the paper size.
[Writing a document theme](01M6CAJG6C3WAZ2MD6W3EGBMKY-writing-a-document-theme.md)
builds one step by step and is the place to start. The `html` format reads
none of this; its look comes from a
[site theme](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md).

## Setting up a theme

Drop a Typst file into `.ntropy/themes/typst/` and name it in the
`[render]` section. `ntropy init` does not create the directory, so make
it first:

```bash
mkdir -p .ntropy/themes/typst
```

```toml
# .ntropy/config.toml
[render]
theme = "corporate"
```

That is the whole setup. From then on every `ntropy render` in the vault
uses it, with nothing extra on the command line:

```bash
ntropy render 01j8za2…              # themed

# and so is every note in the vault
ntropy search -n | tail -n +2 | awk '{print $1}' |
  while read -r id; do ntropy render -n "$id"; done
```

`--theme <name>` overrides the configured theme for one render, and
`--theme default` goes back to ntropy's built-in look. `theme = "default"`
in the config means the same. A theme that is missing or does not
compile fails the render: a document is never quietly produced in the
wrong livery. A configured theme with no file fails before the vault is
scanned, with an error naming the path it looked for. A theme that does
not compile fails with typst's error, described under
[Errors and warnings](#errors-and-warnings).

ntropy ships no named themes of its own beyond the built-in default.
Themes are the vault's own files.

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
| `1. item` | `enum`, with the numbers as written |
| `- [ ] task` | a `list` item that starts with `task(done: ...)` and a space; the list's bullet stays unless the theme changes `list` |
| `> quote` | `quote(block: true)` |
| `> [!NOTE]` and the other four kinds | `callout(kind: "note")`, the kind in lower case |
| pipe table | `table` with a `table.header`; `align` is always passed, from the Markdown alignment row and `auto` where it has none |
| fenced or indented code | `raw` block, with the fence's language |
| `` `code` `` | `raw` inline |
| `*em*`, `**strong**`, `~~gone~~` | `emph`, `strong`, `strike` |
| `[text](url)`, bare URLs | `link` |
| a note link | `link` to `<slug>.pdf` around `notelink`, or `link` to the heading's label when the link names a heading of the note itself |
| `![alt](path)` with a local path | `image` with a path from the vault root |
| `[^label]` | `footnote` |
| `---` | `line(length: 100%)` |
| a hard line break | `linebreak()` |

A column with a Markdown alignment keeps it: `align` is always passed, so
`set table(align: ...)` changes nothing. A column without one follows
`show table.cell: set align(...)`; the built-in look sets it to `start`. A
quote with any other marker, such as `> [!FOO]`, stays a `quote`, so
`callout` only ever sees the five kinds. Raw HTML is dropped, with a warning
naming it.

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
done task box is drawn as a shape, so it needs no font. A glyph a font
lacks is taken from another font typst finds.

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

> [!WARNING]
> A font name typst does not find falls back to the default font, and the
> render succeeds without a message, also under `--strict`. `pdffonts`
> from poppler lists the fonts a PDF embeds; the compile under
> [Errors and warnings](#errors-and-warnings) shows typst's warning.

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

> [!NOTE]
> Because a theme's assets are addressed from the vault root, compiling a
> `--to typst` artifact by hand takes the vault as the root. typst refuses
> a document file outside that root, so hand the artifact over on standard
> input, as ntropy does:
> `typst compile --root <vault> - note.pdf < note.typ`.

## Errors and warnings

A theme that does not compile fails the render with typst's error, which
names where the problem is. `<stdin>` with a line number means a line of
the emitted document, the file `--to typst` writes: the theme starts after
ntropy's own definitions, so that line is further down than in the theme
file. An error in a file the theme imports names that file and its own
line.

ntropy does not pass on typst's warnings. Compiling the `--to typst`
artifact by hand, as in the note above, prints them.

To see a theme draw the parts of a note in one document, render the theme
test note, a download that comes with the note it links to and its two
images.
[Writing a document theme](01M6CAJG6C3WAZ2MD6W3EGBMKY-writing-a-document-theme.md#before-you-start)
gives the command that puts it into a vault, and its last section what to
look at.

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
