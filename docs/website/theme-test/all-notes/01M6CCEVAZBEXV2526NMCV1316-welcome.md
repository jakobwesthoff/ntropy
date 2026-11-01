---
title: Welcome
tags: [tour]
status: published
---
This vault is test material for a site theme. Exported with `ntropy site`,
it gives a site with the kinds of page and the parts of a page that the
built-in theme draws, so a theme can be checked against them. The last
section names the few parts this vault cannot show.

## What to look at

- [Theme test note](01M6CCEVBP701HH0VPBT6ZNAYZ-theme-test-note.md): the
  body of a note with all of its parts, a frontmatter table with nested
  values, an outline with every heading level, related notes.
- [Theme test companion](01M6CCEVCDRQ5504MDH503KQT2-theme-test-companion.md):
  the target of a link with a heading anchor.
- [Site tour](01M6CCEVD1R5XTNFQDV573K5ZQ-site-tour.md): a group page made
  from a note, with the group's notes listed below it, and three tour stops
  in the order the sidebar gives them.
- [Tour detail](01M6CCEVFKD842CP4X1BC0EFMD-tour-detail.md): a breadcrumb
  with three steps.
- [Raw HTML and remote images](01M6CCEVG5ADSSY9A58W98T6V2-raw-html-and-remote-images.md):
  what only web pages show.
- [Hidden note](01M6CCEVGR648RP1RMXKKM3XP6-hidden-note.md): a page that no
  sidebar, listing or pager names.
- [Donaudampfschifffahrtsgesellschaftskapitaensmuetzenabzeichenverleihung](01M6CH3GY5AYJD5Z44FG452SBS-donaudampfschifffahrtsgesellschaftskapitaensmuetzenabzeichenverleihung.md):
  a title, a tag, a frontmatter key and a frontmatter value that are one long
  word each, in the sidebar, the listings, the tag index and the note header.
- The sidebar: the view `by-status` with a group per status, and the tags.
  The tag pages and the view's pages list their notes.
- This page is the front page, set by `index` in the `[site]` table of
  `.ntropy/config.toml`.

## Trying a theme

Run these in the vault's directory. The first command copies the built-in
theme into `.ntropy/themes/site/mine/` as a starting point:

```bash
ntropy site theme init mine
```

Select it in `.ntropy/config.toml`, in the `[site]` table:

```toml
theme = "mine"
```

Export the site with the theme, and once more with the built-in one to
compare:

```bash
ntropy site -o ../theme-test-site --force
ntropy site --theme default -o ../theme-test-site-default --force
```

The `[site.vars]` table of the config is there for a theme's own
templates; the built-in templates do not read it. A note's `site.template`
names a template of the theme that renders that note's page.

## What the vault does not show

- The generated front page: delete the `index` line of the `[site]` table
  to see it.
- A breadcrumb step without a link: it comes from a hand-made group of a
  `[[site.nav]]` table, which replaces the whole sidebar.
- The "No notes." line of a listing with nothing in it.
- The search, the switch between light and dark, and the highlighting of
  code are drawn by the page's scripts, so they show in a browser only.
