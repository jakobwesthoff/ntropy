---
title: Exporting a website
tags: [docs/publish]
site:
  order: 1
---
`ntropy site` turns the vault into a static website: a set of files any
web host serves, and that a browser opens straight from disk, no server
needed. The pages take their look from a site theme; see
[Themes](01M6CJK3JX0YS4R2FFHPJTG0JG-themes.md).

## Export and open

```bash
ntropy site -o ./public              # every note
ntropy site -o ./public tag:public   # only the notes a query selects
open "$(ntropy site -o ./public -p)" # export, then open the front page
```

`-o` is required. A non-empty output directory is refused unless you
pass `--force`, which empties it first. The optional query restricts the
exported notes; a link to a note outside the set stays plain text. `-p`
prints the path of `index.html` and nothing else. Without it, the
command ends with a report naming the directory, the page count, and the
warnings.

The output is plaintext, so writing it inside an encrypted vault prints
a warning. Write it elsewhere.

## What the site contains

The site mirrors the ways you reach a note in the vault:

- Every note is a page under `notes/`.
- The tag hierarchy is a tree of pages under `tags/`, each listing the
  notes carrying the tag or any tag below it.
- Every view becomes a tree under `views/`, nested like its directory. A
  view over `tags` is skipped, since the tag pages already are that
  view.
- Images and files the notes link are copied under `files/`, a linked
  directory with its tree. Links are relative, so the site works from
  `file://` and from any path on a host.

Every page carries:

- a sidebar with the views and the top-level tags, a drawer behind the
  menu button on a phone;
- breadcrumbs;
- an outline of the note's headings that follows your position while
  you scroll;
- a `#` after every heading, shown when you hover or tab to it, whose
  address you can copy to link the section;
- a light/dark/system switch that remembers your choice in the browser.

A note page ends with the notes that share the most tags with it, at
most eight. Tags link to their pages wherever they appear, and note
links point at the target's page.

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
without one, newest first, then the child groups by name. A landing
note's title and body become its group's page, and links to the note go
there. The listing of what the group holds appears below it only when
the note asks for it. A hidden note keeps its page and stays searchable;
it just appears nowhere in the navigation.

Previous and next follow the sidebar's reading order across groups and
sections, so a tag subtree with landing notes reads like a book. A link
into another section is labelled "Previous chapter" or "Next chapter".

`template` names a template of the site theme that renders the note's
pages instead of the built-in `page.html`. That is how a landing page
gets a hero, or an impressum drops the sidebar. A name the theme has no
template for is a warning, and `page.html` is used.

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
related notes from every page, which suits a documentation tree whose
pages all share the same tags. A note's own `site.related` wins over it
either way.

The same table holds `title` (by default the vault directory's name),
`index` (the ULID of the front page's note), `lang` (the page language,
`en` by default), and `theme`, the site theme. A `[site.vars]` table is
free-form: ntropy reads nothing from it, and the theme's templates
receive it as `vars`. That is where a footer's copyright line or a row of header links
comes from. A date, time, or datetime in the table reaches the templates
as text in the form TOML writes it (`2026-11-01`,
`2026-11-01T12:00:00+01:00`).

## Search

The search is a palette over the page, made for readers rather than for
the query language. It runs in the browser over data exported with the
site, so it works from disk.

| Key | Action |
|-----|--------|
| `/`, `Ctrl+K` (`Cmd+K` on a Mac), or the header's search button | Open the search |
| _type_ | Search as you type |
| arrow keys | Move through the results |
| `Enter` | Open the selected result |

A word matches titles, tags, frontmatter values, and text, and two words
need both. `tag:wis` finds `wisdome`, and a matching tag or view page
appears above the notes. The typed terms of the
[Query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) still
narrow: `tag:work and not status:done` works, and `text:` is the same
regex as in the CLI.

## Warnings

Warnings go to stderr, and the site is written anyway. `--strict` makes
them fail the exit code, which is what a build script wants. An export
warns about:

- a referenced file that is missing or lies outside the vault;
- a linked folder that has nothing to copy;
- a link to a file the export never copies (see below);
- a code fence whose language has no highlighting grammar;
- a link to a note outside the exported set;
- a link whose `#heading-id` matches no heading on the target's page;
- a configured index note that is not exported;
- a `site` table that is no mapping, or holds a key of the wrong type;
- a `root` that names no page;
- a nav item the site cannot resolve;
- a `template` the theme does not have.

Metadata the export leaves out of a linked folder, such as `.DS_Store`,
prints as a `note:` line instead, which `--strict` does not count.

## Files an export never copies

Some files are never copied, whether a note links them directly or they
sit in a linked folder or in the site theme, at any depth:

- note tools' configuration: `.ntropy`, `.ntropy-vault`, `.obsidian`;
- version control and its history: `.bzr`, `.fslckout`, `.git`, `.hg`,
  `.jj`, `.pijul`, `.svn`, `CVS`, `_darcs`;
- sync tools' state: `.dropbox.cache`, `.stfolder`, `.stversions`;
- trash: `$RECYCLE.BIN` and every name that starts with `.Trash`;
- credentials: `.aws`, `.env`, `.gnupg`, `.netrc`, `.ssh`;
- file-manager metadata: `.DS_Store`, `Thumbs.db`.

Names match in any letter case, except `CVS`, so a folder `CVs` is
copied. A linked folder also leaves out every hidden file and folder in
it; a hidden file that a note links by name is copied unless it is on
the list. A link to a listed file is a warning and keeps its text in the
page.

Notes the export leaves out are never copied either. A link to
`all-notes/` or to a view's folder copies the files of the exported
notes only, and a link to another note's file keeps its text with a
warning. For this, every file directly in `all-notes/` ending in `.md`
or `.age`, in any letter case, counts as a note.

The site is never copied into itself. An earlier render or site export
stored inside the vault, though, is an ordinary folder, which a link to
a folder holding it copies.

Only files inside the vault are copied. A file or folder that is a
symlink to somewhere else on the machine is left out with a warning,
wherever the symlink sits. That covers an asset folder like
`assets -> ~/Pictures`, and an `all-notes/` that is itself a symlink,
whose notes still become pages but whose images and files beside them
are not copied. The rule has no exceptions, so copy such files into the
vault to have them in the site. A symlink that stays inside the vault
works as any other path.
