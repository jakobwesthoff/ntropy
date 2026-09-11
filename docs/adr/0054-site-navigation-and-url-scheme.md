# 54. Site navigation and URL scheme

Date: 2026-09-11

## Status

Accepted

Amended 2026-09-11, from the design pass on the exported pages (the
user's answers are recorded in `docs/research/web-export/decisions.md`,
Q55 to Q66):

- A view whose field is `tags` is no section and gets no pages; the tag
  section is that view already.
- The sidebar's tag section lists the top-level tags with their note
  counts; the tree below lives on the tag pages. A view section starts
  collapsed unless it holds the current page. On narrow screens the
  sidebar is a drawer behind a menu button.
- A note page's tags link to their tag pages, and the page ends with the
  notes sharing the most tags with it, ancestors counted, at most eight,
  ties newest first.
- A body whose first block is a level-one heading reading exactly the
  note's title has that heading dropped, on the site and in
  `render --to html`.
- The scheme switch has three states, system, light, and dark.
- `/` opens the search with the query box focused; Escape closes it.
- Note lists are one row per note, date, title, and tag links, newest
  first, not grouped.

Defines the pages the export of
[ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md)
writes. Views come from
[ADR 0009](0009-generic-group-by-field-view-definitions.md), tags from
[ADR 0006](0006-hierarchical-tags-by-slash-convention.md), slugs and the
disambiguator from
[ADR 0023](0023-slug-tag-and-disambiguator-normalization-rules.md), the
per-note artifact name from
[ADR 0044](0044-cross-document-links-between-rendered-notes.md).

## Context

A vault is flat. The structure a documentation site shows in its sidebar
has to come from what the vault already has: the configured views, the
tag hierarchy, and the note order. Two notes may share a slug with
different ULIDs, so a slug alone does not name a page. A view may be named
`notes` or `tags`, so view names cannot sit at the site root beside the
note pages.

## Decision

### Front page

`[site] index = "<ulid>"` names the note that becomes `index.html`.
Without it, the export generates an overview: the site title, the note
count, the newest notes, the top-level tags with counts, and the
configured views, each linking into its index page.

`[site] title` defaults to the vault directory name; `[site] lang`, the
`html` `lang` attribute, defaults to `en`. Both are optional.

### Sidebar

The sidebar is built from the configured views and the tag hierarchy.
Each is a collapsible section whose groups nest as the filesystem views
do. There is no hand-curated order. Notes inside a group, on a tag page,
and on a group page are sorted newest first, ULID descending, as the CLI
lists them.

### Pages and paths

One directory per node:

    index.html
    notes/<slug>.html
    tags/index.html
    tags/<a>/<b>/index.html
    views/<name>/index.html
    views/<name>/<group>/<sub>/index.html

A note's page is `notes/<slug>.html`. Only colliding notes are named
`<slug>-<tail>.html`, the tail being the shortest ULID suffix of at least
three characters that separates them, the rule the view leaves use. A tag
page lists the notes carrying the tag or any descendant tag, the `tag:`
query's sub-path rule, and lists its child tags. There is no chronological
all-notes page and no recently-modified page.

### Note page

The page shows the title, the tags as chips, and every other frontmatter
field as key/value beneath, nested values included. It has an outline
built from the note's headings with the current section highlighted while
scrolling, previous and next links, breadcrumbs, and the light/dark
toggle. A note's first view and group in sidebar order define its
previous and next links and its breadcrumb, statically, regardless of how
the reader arrived. There are no backlinks.

### Assets

The export copies the files inside the vault that exported notes
reference through images or links, plus the theme's files. A reference to
a file outside the vault is a warning. Remote images stay remote.

### Rejected alternatives

- **A curated table-of-contents note** defining the sidebar, alone or
  winning over views and tags when present; and **a frontmatter order
  field**. The user named an order override as a topic for a later
  iteration.
- **`notes/<ulid>.html`**, **`notes/<ulid>-<slug>.html`**, and
  **`notes/<slug>/index.html`**; a full ULID suffix on colliders.
- **View names at the site root** as in the vault, and **flat
  `<node>.html` files** beside child directories.
- **Tag pages listing the exact tag only**, children as links.
- **The vault's README as the front page**, alone or above the overview.
- **Previous/next from the page the reader came from**, and
  **chronological previous/next**.
- **A "linked from" section** per note.
- **Copying every non-note file** under `all-notes/` and a vault assets
  directory, and a configurable include list.
- **Showing only title, tags, and date**, or a configured field list.

## Consequences

- A note's URL changes with its title, since the slug does, and a
  collider's URL can change when another collider appears.
- A note that sits in several groups has one breadcrumb and one
  previous/next pair.
- A file a note references but that lies outside the vault is not part
  of the site.
