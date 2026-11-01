---
title: "Heading ids can collide with the ids of the page around them"
kind: bug
component: site
origin: review
---
# Heading ids can collide with the ids of the page around them

On pages exported by `ntropy site` and `render --to html`, a heading's id
can equal an id the page itself uses, so a link to that heading lands on
the wrong element. Found on 2026-11-01 in the adversarial review of the
plan for resolving links to headings in other notes; not reproduced on its own yet.

## Problem

- A heading named "Sidebar" gets the id `sidebar`, which the page
  template also gives the sidebar (`src/site/theme/templates/page.html`,
  `id="sidebar"`). Footnote ids (`fn-N`) and icon ids (`icon-*`) can
  collide the same way.
- Headings inside footnote definitions that no reference uses are counted
  when heading ids are made, although they are not on the page.

## Expected

Every heading id on a page is unique among all ids of that page, and only
headings that are drawn get one.
