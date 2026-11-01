---
title: Markdown flavor
tags: [docs/notes]
site:
  order: 5
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
| Note links | ordinary links targeting a note's filename, optionally followed by `#heading-id`; see [Note format](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md) |
| Images | `![alt](path)` relative to the note |
| Horizontal rules | `---` on its own line |

Deliberately not supported: math (`$x^2$` stays literal text), definition
lists, and emoji shortcodes (`:smile:` stays text).

## Where the output formats differ

The PDF and the web page come from the same walk over the Markdown, but
they cannot carry everything the same way. The terminal drawing of the
`ansi` format comes from the patine library instead.

Raw HTML in a note renders on GitHub. It passes through verbatim into
the `html` format of `render` and into every page of a site export. A
PDF, and the emitted Typst document, drop it, and ntropy prints a
warning naming what it dropped.

Remote image URLs stay `<img>` tags in HTML and on the site, where the
browser fetches them. In a PDF they become links showing the alt text,
again with a warning, because PDF rendering never touches the network.

Local images and files that a note references are copied into the HTML
artifact's files directory and into the site export. A path that starts
with `/` is read from the vault root. A path that leaves the vault, from
the note or from the root, is not copied: the export warns, and the page
keeps the path as written. The same holds for a path an export never
copies, such as `.ntropy` or `.git`; the list is in
[Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md).
A PDF reads the files from the vault: the `typst` compiler runs with the
vault as its root, so a path that leaves the vault cannot be resolved.

A footnote may be referenced more than once, and its definition may
reference itself or other footnotes. HTML shows one numbered marker per
reference and one list entry. A PDF draws the footnote once, at its
first reference in the text, and every other reference shows the same
number.

The `ansi` format, the note drawn for a terminal, drops raw HTML without
a warning and keeps the text between inline tags. It shows an image as
`[image: alt]` with its source after it. It draws the five callouts with
a heavy bar and a bold title after a symbol. Any other `> [!type]` quote
becomes a neutral callout with `○`; PDF and HTML keep it a quote with the
marker as text.

> [!NOTE]
> A `#` in a link or image target always starts the fragment.
> `[the spec](spec.pdf#page=2)` links `spec.pdf` and keeps `#page=2`, so a
> file with a `#` in its name cannot be linked.
