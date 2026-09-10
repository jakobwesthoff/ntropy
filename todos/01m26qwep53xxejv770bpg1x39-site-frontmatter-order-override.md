# Site: note order override through frontmatter

Deferred from the site's sort rule (ADR 0054, `docs/design/site-export.md`).
Notes inside a sidebar group, on a tag page, and on a group page are sorted
newest first, ULID descending. Decision (user, 2026-09-11): "usually newest
first, but we might want to allow override of order via frontmatter unsure
how this could be done. first lets go newest first, but lets keep in mind
that we might want to discuss solve this order problem later on in another
iteration".

## What was discussed

- A frontmatter field such as `order` or `weight` ordering notes inside
  the groups of a chosen view was one of the sidebar options offered and
  not chosen for the first implementation.
- A curated table-of-contents note defining the sidebar, in the style of
  mdBook's `SUMMARY.md`, was offered and not chosen.

## To decide later

- Whether the override is a frontmatter field, a per-view setting in
  `config.toml`, or a curated note.
- How an override interacts with the tag hierarchy pages, which have no
  configured view to attach a setting to.
- Whether the override also affects previous/next links, which follow the
  same order.
