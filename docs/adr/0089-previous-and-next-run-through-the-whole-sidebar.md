# 89. Previous and next run through the whole sidebar

Date: 2026-11-01

## Status

Accepted

Amends [ADR 0056](0056-sidebar-order-labels-landing-notes-and-a-nav-table.md),
whose previous and next links followed the reading order through the whole
tree of one top-level section only.

## Context

With `[site] root = "tags/docs"` every child group of the root is a sidebar
section. The last page of a section had no next link, and the landing note
of a section had no previous or next link at all, since the pager walked
only the items of a section and a section's own landing note is not one of
them.

A note placed in several groups appeared in the walked order more than once.
A later note could then link back to an earlier one, and a note that ended
one group and opened the next was its own next link.

## Decision

Previous and next follow the reading order through the whole sidebar, for
every site, with or without `[site] root`:

- The order reads the sidebar sections one after the other. In a section,
  the section's own landing note comes first, then the items in order, a
  group's landing note before its entries.
- Every note is one stop, at its first position in that order. Previous and
  next are the neighbouring stops. A group without a landing note is no
  stop.
- A note the sidebar does not reach has no previous and next link.
- The landing note of a section has no breadcrumb above it, as its page is
  the section's own.

The template context of a page tells a theme whether a neighbour lies in
another sidebar section:

- `prev` and `next` each carry `section`, the title of the sidebar section
  of the target, and `crosses_section`, true when that section differs from
  the section of the page. The sections are compared by position in the
  sidebar, not by title.
- `title` and `href` stay as they were, so a theme that does not read the
  new fields works unchanged.

The built-in theme marks a link that crosses a section: its label reads
"Previous chapter" or "Next chapter" and the link carries the class
`crosses-section`, which colours the label with the accent colour in
semibold. A theme can mark the link differently, or leave it out.

Two cases fall outside the order:

- The root group of a rooted sidebar has a section only when it holds notes
  of its own or no child groups. Without that section its page is not in the
  sidebar, so its landing note is no stop.
- A front page that is no stop keeps no previous and next link. This holds
  for the generated overview and for an index note the sidebar does not
  reach. The front template of ntropy's own website extends `base.html`,
  which draws no pager.

A landing note that lands several groups renders one page per group with the
previous and next links of its first stop.
