---
title: Rendering to PDF and HTML
tags: [docs/publish]
site:
  order: 1
---
`ntropy render` turns a single note into a document: a typeset PDF, the
Typst source behind it, or a standalone web page. This page covers the
command, its three formats, and how rendered notes link to each other. The
look of a PDF is the subject of
[Document themes](01M28Q32HRVNF9EBZTC69XDZ49-document-themes.md); the look
of the web page comes from a
[site theme](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md).

## Rendering a note

The default output is a PDF with the title, date, and tags up top and the
body below. Links to other notes show the target's current title.

```bash
# Render a note into ./<slug>.pdf in the current directory
ntropy render 01j8za2…

# Name the output yourself, and open the result in one go
open "$(ntropy render -p 01j8za2… -o q3-report.pdf)"
```

`render` takes the selector `search` takes: a full ULID, or a
[query](01M28Q32FC8RW01C5DSEEV77DW-query-language.md). It has to end up
with exactly one note. Several matches open the picker, pre-filtered;
under `-n` an ambiguous selector is an error listing the candidates. With
no selector at all, every note feeds the picker.

Without `-o`, the artifact is `./<slug>.<ext>` in the current directory:
the slug of the note's filename plus `pdf`, `typ`, or `html`. `-p` prints
the artifact's path on stdout and nothing else, which is what makes
`open "$(ntropy render -p ...)"` work. Without `-p`, ntropy announces the
render and ends with a report naming the artifact, the format and engine
that produced it, and its size:
`Rendered quarterly-review.pdf (pdf via typst, 12.4 KiB)`.

ntropy converts the note to Typst with its own engine and only typesets
the PDF with [typst](https://typst.app), so that is the single tool you
install yourself, for example with `brew install typst`. If it is not on
your `PATH`, the render fails with an error naming it.

Content the artifact cannot carry, raw HTML in a PDF or a remote image
(typst fetches nothing from the network), is dropped with a warning on
stderr. Under `--strict`, those warnings fail the command, as scan
warnings do.

## Formats

`--to` picks the format:

- `pdf` is the default.
- `typst` writes the emitted Typst document, a `.typ` file, and needs no
  external tool at all.
- `html` writes the note as a web page, the same page the
  [website export](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md)
  gives it, and needs no tool either. Beside `report.html` it writes a
  `report_files/` directory holding the theme, the fonts, the scripts,
  the grammars its code needs, and the images it shows, the way a
  browser saves a page. Move or send the two together. The page keeps
  the header, the scheme switch, and the outline, and drops what needs
  a site: the sidebar, the search, breadcrumbs, and the pager.

Whatever the format, `render` refuses to overwrite an existing artifact,
or for `html` a non-empty files directory, unless you pass `--force`,
which replaces both.

## Links between rendered notes

A link to another note becomes a real link in the PDF, pointing at
`<slug>.pdf`. That is the target note's slug, which is also the name
`render` gives that note's artifact by default. Render a set of notes
into one directory and their cross-references line up:

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
artifact name, so a link to either reaches whichever was written last.

The `typst` format emits the same link, and it always points at
`<slug>.pdf`, never at a `.typ` file: the Typst document is the source of
a PDF. The `html` format follows the same convention with `<slug>.html`
beside the artifact.

> [!NOTE]
> A rendered artifact is plaintext by nature. Writing one into an
> [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md) means
> it syncs unencrypted, and ntropy warns when the output path lands
> there. That happens most often when your shell is sitting in the vault
> and the default `./<slug>.pdf` applies. The warning does not change the
> exit code.
