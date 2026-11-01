---
title: "Previous and next stop at the end of a sidebar section"
kind: improvement
component: site
origin: discussion
tags: [ux]
---
# Previous and next stop at the end of a sidebar section

On an exported site, the previous/next pager never leaves a sidebar
section, and a section's landing page has no pager at all. A reader
cannot click through the docs from front to back: the last page of
"Getting started" on the ntropy website has no "Next".

## Problem

`src/site/model.rs` builds the pager order per sidebar section (around
lines 1130-1155 on 0e27286). With `[site] root = "tags/docs"` every docs
group is its own section, and its landing is the section page, not an
entry (the landing is pushed only for nested groups, around line 1102).
This is the designed behaviour of ADR 0056 ("of a top-level section")
and `docs/design/site-export.md` (around line 250, "through the whole
tree of the sidebar section"). Repro: `ntropy --vault docs/website -n
site -o <dir> --force`, then look at the `.pager` nav of
`notes/commands.html`, `notes/configuration.html` and any
`tags/docs/*/index.html`. Found by the docs consolidation on 2026-11-01.

## Decision

The user decided on 2026-11-01 (AskUserQuestion in the lead session of
the todo run): the pager runs through the whole sidebar tree, across
section boundaries and through the section landing pages, so a reader
can click through the docs like a book. This holds for every site, with
or without `[site] root`.

The user added the same day: a previous or next link that crosses into
another section (chapter) is marked as such, so the reader sees that the
chapter changes. How the mark looks is shown to the user with shots
before it is final.

And: the template context tells a theme whether a previous or next link
crosses into another section, so a template can decide how to show it on
that basis, or leave such a link out entirely.

## Suggested fix

Build one pager order over the whole sidebar in its reading order, with
each section's landing page before its entries. A new ADR amends ADR
0056 (0056 only gains "Amended by"); update `docs/design/site-export.md`
and the website note "Exporting a website".
