# 56. Sidebar order, labels, landing notes, and a nav table

Date: 2026-09-11

## Status

Accepted

Amended 2026-09-11, after the first export of a documentation vault
(research log, Q85): `[site] related` switches the related notes off
for the site, and a note's `site.related` overrides it for its page.

Amends [ADR 0054](0054-site-navigation-and-url-scheme.md), whose
sidebar had no hand-curated order and whose rejected alternatives
included a frontmatter order field. The user's answers are recorded in
`docs/research/web-export/decisions.md`, Q77 to Q84. Views come from
[ADR 0009](0009-generic-group-by-field-view-definitions.md), tags from
[ADR 0006](0006-hierarchical-tags-by-slash-convention.md), the query
DSL's `tag:` predicate from
[ADR 0012](0012-query-dsl-with-hand-rolled-parser.md), the
`[site]` table from [ADR 0016](0016-configuration-format-location-and-vault-resolution.md).

## Context

The goal is a documentation site with a sidebar like a Starlight site's:
sections listed by hand under invented labels, each an ordered mix of
pages and labelled groups, a group's landing page first, some pages kept
out of the navigation. The export builds its sidebar from the configured
views and the tag tree, groups alphabetical, notes newest first, and has
no way to name, order, or hide anything. A tag subtree already is the
directory tree such a site has, so most of the structure exists; what is
missing is order, labels, landing pages, a root below the vault, and a
way to assemble a top level from several places.

## Decision

### The `site` table in a note's frontmatter

A note may carry a `site` mapping with these keys, each optional:

- `order`, an integer: the note's position among the entries of every
  group holding it.
- `label`, a string: the name the sidebar and the previous/next links
  show for the note; the page keeps the title.
- `hidden`, a boolean: the note appears in no sidebar, no list (front
  page, tag and group pages, related notes), and as nobody's previous or
  next; its page is exported, linkable, and in the search data.
- `index`, a boolean: the note is the landing note of every group it is
  a member of.
- `related`, a boolean: whether the note's page ends with its related
  notes, whatever the site's setting. The group's page renders the note's title and body, then
  the child groups and the remaining notes; the note has no page of its
  own, and links to it go to the group page (the first such group in
  sidebar order when there are several). The note's `label` names the
  group; its `order` places the group among the parent's entries.

`site` is reserved by these rules and is not shown as a frontmatter
field on the page. A key of the wrong type is an export warning and is
ignored.

### Order within a group

The entries of a group are its notes and its child groups together. The
entries with an `order` come first, ascending; then the notes without
one, newest first; then the groups without one, by label. The sidebar
lists the entries in that order, and previous and next follow the
reading order through the whole tree of a top-level section, crossing
group boundaries, hidden notes skipped. A group page lists its
descendants in the same reading order.

### Related notes

`[site] related`, `true` by default, says whether note pages end with
their related notes; a note's `site.related` overrides it.

### The nav root

`[site] root` names a page of the site, `tags/<path>` or
`views/<name>/<group>`, and the sidebar shows that group's entries as
its top level; breadcrumbs start there. Without the key, an export whose
query is a single `tag:` predicate is rooted at that tag; otherwise the
sidebar is the one of ADR 0054, sections for the views and the tags.

### The nav table

`[[site.nav]]` lists sections, each with a `label` and `items`. An item
is one of:

- `{ note = "<ulid>" }`, a note;
- `{ tag = "<path>" }`, the tag's subtree as a group;
- `{ view = "<name>" }` or `{ view = "<name>", group = "<value>" }`, a
  view or one of its groups as a group;
- `{ tags = true }`, the whole tag tree as a group;
- `{ label = "…", items = [ … ] }`, a group assembled by hand.

Every item takes an optional `label`, which wins over a landing note's.
An item's position is its position in `items`; inside an expanded
subtree the order above applies. When the table is present it is the
whole sidebar; views and tags appear only through items. A note that is
not exported, and a tag or group with no exported note, are export
warnings and the item is left out.

### Rejected alternatives

- **`order = 0` marking the landing note.** One number cannot both mark
  the landing note and place its group among the parent's entries.
- **A curated table-of-contents note**, again; **a nav table only**,
  with no frontmatter; **frontmatter and a root only**, with no way to
  mix sources at the top level.
- **Curated sections above the automatic ones**, and **the tag section
  always appended** under a nav table.
- **Hiding from the navigation only**, and **hiding from search and
  related notes too**.
- **Dropping the related notes automatically** under a root or a nav
  table, and **hiding them by theme CSS only**.
- **A landing note keeping its own page**, and **the note replacing the
  group page** without the listing.
