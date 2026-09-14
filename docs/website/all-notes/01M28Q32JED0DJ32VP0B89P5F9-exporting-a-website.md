---
title: Exporting a website
tags: [docs/publish]
site:
  order: 3
---
`ntropy site` turns the vault into a static website: a set of files any
web host serves, and that a browser opens straight from disk, no server
needed. This page covers the command, what the site contains, and how
the `site` table in a note's frontmatter and the `[site]` config shape
the navigation. The look of the pages is the subject of
[Site themes](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md).

## The site command

```bash
ntropy site -o ./public              # every note
ntropy site -o ./public tag:public   # only the notes a query selects
open "$(ntropy site -o ./public -p)" # export, then open the front page
```

`-o` is required. A non-empty output directory is refused unless you
pass `--force`, which empties it first. The optional
[query](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) restricts the
exported notes; a link to a note outside the set stays plain text. `-p`
prints the path of `index.html` and nothing else. Without it the command
ends with a report naming the directory, the page count, and the
warnings.

Warnings go to stderr, and the site is written anyway; `--strict` makes
them fail the exit code, which is what a build script wants. An export
warns about a referenced file that is missing or lies outside the vault,
a code fence whose language has no highlighting grammar, a link to a note
outside the exported set, a configured index note that is not exported,
a `site` table that is no mapping or holds a key of the wrong type, a
`root` that names no page, a nav item the site cannot resolve, and a
`template` the theme does not have.

The output is plaintext, so writing it inside an
[encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md) prints
a warning. Write it elsewhere.

## What the site contains

The site mirrors the ways you reach a note in the vault. Every note is a
page under `notes/`. The tag hierarchy is a tree of pages under `tags/`,
each listing the notes carrying the tag or any tag below it. Every
[materialized view](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md)
becomes a tree under `views/`, nested like its directory; a view over
`tags` is skipped, since the tag pages already are that view. Images and
files the notes link are copied under `files/`, a linked directory with
its whole tree. Links are relative, so the site works from `file://` and
from any path on a host.

Each page carries a sidebar with the views and the top-level tags,
breadcrumbs, an outline of the note's headings that follows your
position while you scroll, and previous/next links that follow the
sidebar's reading order. A note page ends with the notes that share the
most tags with it, at most eight. Tags link to their pages everywhere
they appear. Note links point at the target's page. Every page has a
light/dark/system switch that remembers your choice in the browser. On
a phone the sidebar is a drawer behind the menu button.

Code blocks are highlighted in the browser for 72 languages, each page
loading only the grammars its code needs. A fence language without one
is an export warning, and the block stays plain.

The front page is the note named by `[site] index` in the vault config,
or a generated overview of the ten newest notes, the top-level tags, and
the views.

## Shaping the navigation from frontmatter

A note shapes its place in the navigation through a `site` table in its
frontmatter, every key optional:

```yaml
---
title: Basics
tags: [docs/start]
site:
  index: true            # this note is the landing page of docs/start
  listing: false         # true lists the group's contents below it
  label: Getting Started # what the sidebar calls it (and its group)
  order: 1               # its position; on a landing note, the group's
  hidden: false          # true keeps a note out of the sidebar and lists
  related: false         # no related notes under this page
  template: splash       # render with the theme's templates/splash.html
---
```

Inside a group, the entries with an `order` come first, then the notes
without one newest first, then the child groups by name. A landing
note's title and body become its group's page, with the listing of what
the group holds below only when the note asks, and links to the note go
there. A hidden note keeps its page and stays searchable; it just
appears nowhere in the navigation. Previous and next follow the
sidebar's reading order across groups, so a tag subtree with landing
notes reads like a book.

`template` names a template of the site theme that renders the note's
pages instead of the built-in `page.html`, which is how a landing page
gets a hero or an impressum drops the sidebar. What such a template
receives is described under
[Site themes](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md). A name the
theme has no template for is a warning, and `page.html` is used.

The `site` table is never shown as a frontmatter field on the page.

## The sidebar and the vault config

Where the sidebar starts, and what it holds, is the vault config's
business. `root` starts it at one tag or view group instead of the whole
vault, which is what a documentation site wants; exporting with a single
`tag:` query does the same without config. A `[[site.nav]]` table
assembles the sidebar by hand instead, and is then all of it:

```toml
[site]
root = "tags/docs"                   # the sidebar is the docs subtree
related = false                      # no related notes under the pages

[[site.nav]]                         # or: sections listed by hand
label = "Getting Started"
items = [
  { note = "01ARZ3NDEKTSV4RRFFQ69G5FAV" },          # a note, by ULID
  { label = "Gadgets", tag = "docs/start/gadgets" }, # a tag's subtree
]

[[site.nav]]
label = "Reference"
items = [
  { view = "by-status", group = "open" },  # one group of a view
  { view = "by-status" },                  # a whole view
  { tags = true },                         # the whole tag tree
  { label = "More", items = [ ] },         # a group made by hand
]
```

Every item takes an optional `label`, which wins over a landing note's.
An item that names something the export does not have is a warning and
is left out, so the site is still written. `related = false` drops the
related notes from every page, which a documentation tree wants, since
its pages all share the same tags; a note's own `site.related` wins over
it either way.

The same table holds `title` (defaults to the vault directory name),
`index` (the ULID of the front page's note), `lang` (the page language,
`en` by default), and `theme`. A `[site.vars]` table is free-form:
ntropy reads nothing from it, and the theme's templates receive it as
`vars`, which is where a footer's copyright line or a row of header
links comes from. Both are shown in
[Site themes](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md).

## Search

The search is a palette over the page: the header's button, `/`, or
Ctrl+K (Cmd+K on a Mac) opens it, typing searches as you go, the arrow
keys and Enter open a result. It is made for readers rather than for the
query language: a word matches titles, tags, frontmatter values, and
text, two words need both, `tag:wis` finds `wisdome`, and a matching tag
or view page appears above the notes. The typed terms of the
[query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) still
narrow, `tag:work and not status:done` works, and `text:` is the same
regex as in the CLI. The search runs in the browser over data exported
with the site, so it works from disk.

## One note as a page

`ntropy render --to html` renders a single note to the same page on its
own, with a files directory beside it holding what the page needs. The
details are under
[Rendering to PDF and HTML](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md).
