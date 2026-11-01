---
title: Markdown flavor
tags: [docs/notes]
site:
  order: 2
---
Notes are GitHub-flavored Markdown. What GitHub renders is what ntropy
understands, so a vault reads the same on github.com, in your editor's
preview, in a rendered PDF, and on an exported website. This page lists the
supported syntax and the few places where the output formats differ.

## Supported syntax

| Feature | Syntax |
| :--- | :--- |
| Headings | `#` through `######` |
| Emphasis, strong, strikethrough | `*em*`, `**strong**`, `~~gone~~` |
| Inline code, code blocks | `` `code` ``, fenced ` ``` ` blocks with a language tag for syntax highlighting |
| Lists | `-` bullets, `1.` numbered (the starting number is kept), nesting by indentation |
| Task lists | `- [ ]` open, `- [x]` done |
| Tables | pipe tables with `:---`, `:---:`, `---:` column alignment |
| Block quotes | `>` prefixed lines |
| Callouts | `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]` |
| Footnotes | `[^label]` references with `[^label]: text` definitions, in any order |
| Links | `[text](url)`, bare URLs and `www.` hosts autolink, `<mail@example.org>` |
| Note links | ordinary links targeting a note's filename, optionally followed by `#heading-id`, described under [Note format](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md) |
| Images | `![alt](path)` relative to the note |
| Horizontal rules | `---` on its own line |

Deliberately not supported: math (`$x^2$` stays literal text), definition
lists, and emoji shortcodes (`:smile:` stays text).

## Where the output formats differ

The same Markdown walk feeds the PDF and the web page, but they cannot
carry everything the same way. The terminal drawing of the `ansi` format
comes from the patine library instead.

Raw HTML in a note renders on GitHub and passes through verbatim into the
`html` format of [`render`](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md)
and into every page of the [website
export](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md). When rendering to
PDF (or to the emitted Typst document) it is dropped, and ntropy prints a
warning naming what it dropped.

The `ansi` format, the note drawn for a terminal, drops raw HTML without
a warning and keeps the text between inline tags. It shows an image as
`[image: alt]` with its source after it. It draws the five callouts with a
heavy bar and a bold title after a symbol, and any other `> [!type]` quote
as a neutral callout with `○`, where PDF and HTML keep it a quote with the
marker as text.

Remote image URLs stay `<img>` tags in HTML and on the site, where the
browser fetches them. In a PDF they become links showing the alt text,
again with a warning, because PDF rendering never touches the network.

Local images and files referenced from a note are copied into the HTML
artifact's files directory and into the site export. A path that starts
with `/` is read from the vault root. A path that leaves the vault, from
the note or from the root, is not copied: the export warns and the page
keeps the path as written. The same holds for a path the export never
copies, such as `.ntropy` or `.git`
([Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md)
lists them). Rendering to PDF reads
them from the vault: the `typst` compiler runs with the vault as its root,
so a path that leaves the vault cannot be resolved.
