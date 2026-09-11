---
title: Publishing
tags: [docs/publish]
site:
  index: true
  label: Publishing
  order: 4
---
Notes leave the vault in three shapes. `ntropy render` turns one note into
a typeset PDF, or into the Typst document behind it. `ntropy render --to
html` turns one note into a standalone web page with a files directory
beside it, the way a browser saves a page. `ntropy site` turns the whole
vault, or the subset a query selects, into a static website that any file
host serves and a browser opens straight from disk.

[Rendering to PDF and HTML](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md)
covers the `render` command, its three formats, and how rendered notes
link to each other. [Document themes](01M28Q32HRVNF9EBZTC69XDZ49-document-themes.md)
is about the look of a PDF: a Typst file in the vault that redefines the
page, the callouts, the note links, and the checkboxes, and the paper size.

[Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md)
covers the `site` command, the pages it writes, and how the `site` table
in a note's frontmatter and the `[site]` config shape the sidebar.
[Site themes](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md) is the contract a
theme author writes against: the stylesheet's custom properties, fonts,
icons, the page markup, and the templates. The same theme styles the
website and the standalone HTML page.
