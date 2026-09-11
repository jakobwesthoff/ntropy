---
name: site
description: >-
  Export a vault, or a query's subset of it, as a static website with
  `ntropy site`; render one note as the same page on its own with
  `render --to html`; configure the front page, the sidebar's root, a
  hand-assembled nav table, and per-note order, labels, landing pages, and
  hidden pages through the `site` frontmatter table.
metadata:
  tags: site, website, export, html, navigation, sidebar, nav, frontmatter, render
---

# Publishing notes as a website

`ntropy site` turns the vault into a directory of static pages that any file
host serves and a browser opens straight from disk. `ntropy render --to html`
produces the same page for one note, with a files directory beside it. The
look comes from a site theme ([site-themes.md](site-themes.md)); the
structure comes from tags, views, and the `site` table in a note's
frontmatter, all described here.

## The site command

```bash
ntropy site -n -o ./public                    # every note
ntropy site -n -o ./public tag:handbook       # only the notes a query selects
ntropy site -n -o ./public --force            # empty a non-empty ./public first
page=$(ntropy site -n -o ./public -p)         # -p prints public/index.html and nothing else
ntropy site -n -o ./public --theme default    # the built-in look, whatever the config says
ntropy site theme init mine                   # copy the built-in theme to .ntropy/themes/site/mine/
```

- `-o <dir>` is required. A non-empty directory is refused; `--force` empties
  it before writing. Nothing else is deleted.
- The optional query is the ordinary query language
  ([querying.md](querying.md)). A link from an exported note to a note the
  query left out stays as plain text and is reported as a warning.
- `--theme <name>` overrides `[site] theme` for one export; `default` names
  the built-in theme. A theme that does not exist fails the export before
  anything is written.
- Without `-p` the command ends with one report line:
  `Exported 35 pages to public (32 notes, 0 warnings)`.
- Warnings go to stderr, prefixed `warning:`; the site is written anyway.
  `--strict` makes them fail the exit code, which is what a build script
  wants. The warnings an export raises:
  - a referenced image or file that is missing, or lies outside the vault;
  - a link to a note outside the exported set;
  - a code fence whose language has no highlighting grammar (the block
    stays plain);
  - a `[site] index` note that is not among the exported notes (the front
    page becomes the generated overview);
  - a note's `site` table that is not a mapping, or a key of the wrong type
    (the key is ignored);
  - a `[site] root` that names no page of the site (the sidebar falls back
    to the default);
  - a `[[site.nav]]` item the site cannot resolve (the item is left out);
  - a `site.template` naming a template the theme does not have
    (`page.html` is used).
- The export reads every note, so an encrypted vault must be unlocked. The
  output is plaintext, and writing it inside an encrypted vault prints a
  warning. Write it elsewhere.

## What the site contains

| Path | Content |
|------|---------|
| `index.html` | the note named by `[site] index`, or a generated overview: title, note count, the newest notes, the top-level tags, the views |
| `notes/<slug>.html` | one page per note; colliding slugs get a short ULID tail |
| `tags/index.html`, `tags/<a>/<b>/index.html` | the tag tree and one page per tag, listing the notes under it and its child tags |
| `views/<name>/index.html`, `views/<name>/<group>/index.html` | one index per configured view and one page per group; a view over `tags` gets none (the tag pages are that view) |
| `assets/` | the theme's files, the page scripts, the search data, the grammars the site's code needs |
| `files/<vault path>` | images and files the notes reference, copied from the vault |

Every page has the sidebar, breadcrumbs, an outline of the note's headings,
previous/next links, a light/dark/system switch, and the search (a palette
opened by the header button, `/`, or Ctrl+K). A note page ends with the
notes sharing the most tags with it unless that is switched off. Links are
relative, so the site works from `file://` and from any path on a host.

## Configuration: the `[site]` table

In `<vault>/.ntropy/config.toml`, every key optional:

```toml
[site]
theme = "mine"                       # .ntropy/themes/site/mine/; absent = built-in
index = "01ARZ3NDEKTSV4RRFFQ69G5FAV" # the note that becomes index.html, by ULID
title = "Team Handbook"              # defaults to the vault directory name
lang = "en"                          # the html lang attribute
related = false                      # no "related notes" under the pages (default true)
root = "tags/handbook"               # the sidebar starts at this tag or view group

[site.vars]                          # free-form; only the theme's templates read it
copyright = "Acme, 2026"
```

**`root`** names a page of the site, `tags/<path>` or
`views/<name>/<group>`, and the sidebar shows that group's child groups as
its sections, each titled by the group and linking to its page, with the
root's own notes first; breadcrumbs start at the section. Without it, an
export whose query is a single `tag:` term is rooted at that tag; otherwise
the sidebar lists every configured view and the tag tree.

**`[[site.nav]]`** assembles the sidebar by hand instead. When present it is
the whole sidebar; views and tags appear only through items:

```toml
[[site.nav]]
label = "Getting Started"
items = [
  { note = "01ARZ3NDEKTSV4RRFFQ69G5FAV" },              # a note, by ULID
  { note = "01BRZ3NDEKTSV4RRFFQ69G5FAV", label = "Setup" },
  { label = "Gadgets", tag = "handbook/start/gadgets" }, # a tag's subtree as a group
]

[[site.nav]]
label = "Reference"
items = [
  { view = "by-status", group = "open" },  # one group of a view
  { view = "by-status" },                  # a whole view
  { tags = true },                         # the whole tag tree
  { label = "More", items = [ ] },         # a group made by hand, nesting items
]
```

Every item takes an optional `label`, which wins over a landing note's. A key
outside that set does not parse, and the export fails; an item naming a note
that is not exported (or is hidden), a tag or view group with no page, or an
unconfigured view is a warning and is left out.

## Shaping the navigation from frontmatter

A note's `site` table is reserved for navigation and never shown as a
frontmatter field on the page. Every key is optional:

```yaml
---
title: Welcome
tags: [handbook/start]
site:
  index: true            # this note is the landing page of handbook/start
  label: Getting Started # what the sidebar calls it, and its group
  order: 1               # position among the group's entries; on a landing note, the group's
  listing: false         # true lists the group's contents below a landing note
  hidden: false          # true keeps the note out of the sidebar, lists, and pager
  related: false         # switches the related notes off (or on) for this page only
  template: splash       # render with the theme's templates/splash.html (site-themes.md)
---
```

- **Order.** Inside a group, entries with an `order` come first, ascending;
  then notes without one, newest first; then child groups by name. A landing
  note's `order` places its whole group among the parent's entries.
- **Landing notes.** `index: true` makes the note the page of every group it
  is in: the group page shows the note instead of a listing (add
  `listing: true` to keep the listing below it), links to the note go there,
  and the note has no page under `notes/`. Its `label` renames the group.
- **Hidden notes** keep their page and stay searchable; they appear in no
  sidebar, list, or previous/next link.
- **Labels** on ordinary notes rename them in the sidebar and the pager; the
  page keeps the title.
- Previous and next follow the sidebar's reading order across groups, so a
  tag subtree with landing notes reads like a book.

The frontmatter rules for everything else are in
[writing-notes.md](writing-notes.md).

## Workflow: publish a documentation subtree

A documentation site is a tag subtree with a landing note per section. The
tag hierarchy is the directory tree the site shows.

1. Choose a root tag and mirror the sections below it: `handbook/start`,
   `handbook/start/gadgets`, `handbook/reference`.
2. Author every page with `new --empty` and `write`
   ([writing-notes.md](writing-notes.md)), carrying the section tag and a
   `site` table. Give each section a landing note with `index: true`, a
   `label`, and an `order`; give the pages inside an `order`; hide what
   must not appear.
3. Look the ULIDs up before writing config: `ntropy search -n tag:handbook`
   prints them in the first column.
4. Configure `[site]`: `title`, `index` (the front page's ULID),
   `root = "tags/handbook"`, and `related = false`, since documentation
   pages all share the same tags and the related list would repeat the
   sidebar. Add `[[site.nav]]` tables only when the sidebar must mix
   sources or differ from the tag tree.
5. Export with `--strict` and read the report:
   `ntropy site -n -o ./public --strict tag:handbook`.
6. Open `public/index.html` in a browser, or hand the directory to any
   static host.

```bash
# A section landing note and one page inside it.
path=$(ntropy new --empty -p Welcome)
ntropy write "$path" <<'NOTE'
---
title: Welcome
tags: [handbook/start]
site: { index: true, label: Getting Started, order: 1 }
---
# Welcome

What the handbook covers and where to begin.
NOTE

path=$(ntropy new --empty -p Installation)
ntropy write "$path" <<'NOTE'
---
title: Installation
tags: [handbook/start]
site: { order: 1 }
---
# Installation

Steps.
NOTE
```

## Workflow: one note as a standalone page

```bash
ntropy render -n -p 01ARZ3NDEKTSV4RRFFQ69G5FAV --to html -o report.html
```

writes `report.html` and `report_files/` beside it, the way a browser saves
a page: the theme's stylesheet and fonts, the page script, the grammars the
page's code needs, and the images and files the note references. Move or
send the two together. The page keeps the header (titled after the note),
the light/dark switch, and the outline; it has no sidebar, search,
breadcrumbs, pager, or related notes, and its tags are plain text. Note
links target `<slug>.html` beside the artifact, so a set of notes rendered
into one directory cross-references itself. No external tool is needed.

`render` refuses an existing artifact of any format, and a non-empty
`<stem>_files/`, unless `--force` replaces them. Warnings are the export's
(missing or out-of-vault files, a fence language without a grammar).

## Do / don't

| DON'T | DO |
|-------|----|
| Put content under a `site:` frontmatter key | keep `site` for `order`, `label`, `hidden`, `index`, `listing`, `related` only |
| `index = "Welcome"` or `{ note = "welcome" }` | the full 26-character ULID, looked up with `ntropy search -n` |
| Number tags to force an order (`01-start`) | `site.order` on the notes; a landing note's order places its group |
| Hand over a site whose export printed warnings | fix what they name, or export with `--strict` in scripts |
| `ntropy site -o ./public` into a used directory | `--force`, on purpose, or a fresh directory |
| Write the site into an encrypted vault | a directory outside it; the pages are plaintext |
