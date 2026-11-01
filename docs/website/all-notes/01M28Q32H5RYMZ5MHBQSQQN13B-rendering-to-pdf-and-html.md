---
title: Rendering to PDF and HTML
tags: [docs/publish]
site:
  index: true
  label: Publishing
  order: 4
---
Publishing starts with a single note: `ntropy render` turns it into a
typeset PDF, the Typst document behind it, a standalone web page, or the
note drawn for a terminal. For the whole vault as a website, see
[Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md).

## Render a note to PDF

A PDF needs [typst](https://typst.app) on your `PATH`; ntropy converts the
note to Typst itself and only typesets with it. Install it, for example,
with `brew install typst`; the Homebrew formula for ntropy installs it
along. Without it, the render fails with an error naming it.

```bash
# Render a note into ./<slug>.pdf in the current directory
ntropy render 01j8za2…

# Name the output yourself, and open the result in one go
open "$(ntropy render -p 01j8za2… -o q3-report.pdf)"
```

The PDF shows the title up top, the other frontmatter fields below it
with the tags as chips, and the body below that. Links to other notes
show the target's current title.

Without `-p`, ntropy announces the render and ends with a report naming
the artifact, the format and engine that produced it, and its size:
`Rendered quarterly-review.pdf (pdf via typst, 12.4 KiB)`. With `-p` it
prints the artifact's path on stdout and nothing else, which is what
makes `open "$(ntropy render -p ...)"` work.

## Choosing the note

`render` takes the selector `search` takes: a full ULID, or a query. It
has to end up with exactly one note. Several matches open the picker,
narrowed to them; under `-n` an ambiguous selector is an error listing
the candidates. With no selector at all, every note feeds the picker;
under `-n` that is an error asking for a selector.

## Where the artifact goes

Without `-o`, the artifact is `./<slug>.<ext>` in the current directory:
the slug of the note's file name plus `pdf`, `typ`, `html`, or `ansi`. In
an encrypted vault, whose file names carry no slug, the slug comes from
the title. With `-o`, a missing parent directory of the path is created,
whichever format you render.

`render` refuses to overwrite an existing artifact, or for `html` a
non-empty files directory, unless you pass `--force`, which replaces
both.

> [!NOTE]
> A note file that ntropy did not name, such as one from a cloned vault,
> can carry characters a terminal acts on, or start its slug with `-`.
> For such a file, `render` without `-o` stops before writing anything.
> It says to run `ntropy reconcile`, which gives the note its canonical
> name, or to name the artifact with `-o`.

## Formats

`--to` picks the format:

- `pdf` is the default. The built-in look sets Latin, Greek, Cyrillic
  and Hebrew text in the fonts typst brings along; other scripts use the
  fonts of the machine, see Fonts in
  [Document themes](01M28Q32HRVNF9EBZTC69XDZ49-document-themes.md).
- `typst` writes the emitted Typst document, a `.typ` file, and needs no
  external tool at all.
- `html` writes the note as a web page, the same page a website export
  gives it, and needs no tool either. Beside `report.html` it writes a
  `report_files/` directory, the way a browser saves a page. It holds
  the theme, the fonts, the scripts, the grammars the page's code needs,
  and the images it shows. A linked folder is copied without any note
  but this one, and without the files a website export never copies,
  such as `.git` and hidden files. The page keeps the header, the scheme
  switch, and the outline, and drops what needs a site: the sidebar, the
  search, breadcrumbs, and the pager.
- `ansi` writes the note drawn for a terminal at 80 columns: the
  frontmatter as a highlighted block, then the body with its headings,
  emphasis, lists, tables, quotes, callouts and highlighted code styled
  by ANSI escape sequences. `less -R report.ansi` or `cat report.ansi`
  shows it. A link, a link to another note included, shows its target
  dimmed after the text; an image shows as `[image: alt] (url)`. Raw HTML
  is left out without a warning. Control characters in the note are
  shown escaped, as `\u{1b}`. It needs no tool and takes no theme:
  `--theme` with `--to ansi` is an error. `ntropy show` prints the same
  drawing straight to the terminal, at its width.

> [!NOTE]
> An `html` page needs its files directory: move or send `report.html`
> and `report_files/` together.

A PDF drops raw HTML and shows a remote image as a link, each with a
warning; [Markdown flavor](01M28Q32B13V8F4Z4CHE258TC5-markdown-flavor.md)
lists what each format leaves out. typst's own warnings, such as one for
a font it does not find, are shown as warnings of the render too. Under
`--strict`, all of them fail the command.

## Links between rendered notes

A link to another note becomes a real link in the PDF, pointing at
`<slug>.pdf`. That is the target note's slug, which is also the name
`render` gives that note's artifact by default. Render a set of notes
into one directory, and their cross-references line up:

```bash
# Rendered with no -o, each artifact is named after the note's own slug,
# which is exactly what the other one's link points at
cd out
ntropy render 01j8za2…   # writes ./architecture.pdf
ntropy render 01j8zb7…   # writes ./deployment.pdf
```

The link is a plain relative reference, so whether a click opens the
other file is up to your PDF viewer. It finds nothing if the target was
never rendered, or was rendered into a different directory or under a
different name. Two notes whose slugs are identical also share an
artifact name, so a link to either reaches whichever was written last. A
link to a note whose file name `render` refuses to use leads nowhere
until `ntropy reconcile` renames that note.

The `typst` format emits the same link, and it always points at
`<slug>.pdf`, never at a `.typ` file: the Typst document is the source
of a PDF. The `html` format follows the same convention with
`<slug>.html` beside the artifact.

A link with a heading anchor, `[Inside tmux](<ulid>-<slug>.md#inside-tmux)`,
shows the text you wrote instead of the note's title.

- The `html` format appends the anchor to the target,
  `<slug>.html#inside-tmux`. It does not check the anchor, as it does
  not read the other note.
- A link in a PDF cannot address a heading in another PDF, so there the
  link opens the target's PDF and the anchor is dropped.
- When the target is the note being rendered, the PDF link jumps to the
  heading instead.
- A link to a heading of the same note, `[back](#lists)`, works in a PDF
  and in HTML too. In a PDF, `#` or `#top` goes to the top of the first
  page.
- In a PDF, an anchor of the note itself that names no heading shows the
  link text only, with a warning, so `--strict` fails.

> [!NOTE]
> A rendered artifact is plaintext by nature, so one written into an
> encrypted vault syncs unencrypted. ntropy warns when the output path
> lands there, most often when your shell sits in the vault and the
> default `./<slug>.pdf` applies. The warning does not change the exit
> code.

## The look

A PDF takes its look from a document theme, and the `html` page from a
site theme; see [Themes](01M6CJK3JX0YS4R2FFHPJTG0JG-themes.md).
