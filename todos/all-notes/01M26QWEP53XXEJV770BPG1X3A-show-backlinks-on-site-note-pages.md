---
title: "Show backlinks on site note pages"
kind: feature
component: site
status: needs-discussion
origin: discussion
---
# Show backlinks on site note pages

Each note page of the static site should list the notes that link to it, a
"linked from" section. This was left out of the first site implementation
(ADR 0054, `docs/design/site-export.md`) and deferred from that note page
design.

## Decisions

- 2026-09-11, user: backlinks are not in the first implementation.

## Context

Nothing in the library computes backlinks today. ADR 0028 states the intent:
compute them on demand by scanning bodies for the target ULID.

## Proposal

Compute a "linked from" section per note page at export, from the resolved
link tables of all exported notes. The export already builds every note's
link table, so the inverse index is a fold over them.

## Open questions

- Whether the computation lives in the library, so the LSP and CLI todos
  reuse it.
- Whether links from notes outside the exported set count. ADR 0046's query
  filter selects that set.

## Relations

- Relates to: [Show backlinks through textDocument/references](01KVZKK1BVQNHFX3V6W7W80YTD-show-backlinks-through-textdocumentreferences.md), the same backlink computation
- Relates to: [Add CLI commands to insert links and list backlinks](01KW4BDQKQFN8EP41CET8B1AVZ-add-cli-commands-to-insert-links-and-list-backlinks.md), the same backlink computation
