---
title: "Add CLI commands to insert links and list backlinks"
kind: feature
component: cli
origin: discussion
---
# Add CLI commands to insert links and list backlinks

Out-of-band link commands for users who do not run the language server.
Deferred during the first linking iteration, which shipped the link format
(ADR 0028) and the language server.

## Proposal

- A `link` command: a picker selects a target, and the command emits or copies
  the link markup `[display](<ulid>-<slug>.md)`.
- A `backlinks <id>` command: scan note bodies for references to the ULID.

## Open questions

- Is backlink performance over large vaults acceptable under the stateless
  model (ADR 0002)? The `backlinks` command scans all bodies on each invocation
  with no persistent cache, unlike the language server's in-memory session scan
  cache.

## Relations

- Relates to: [Show backlinks through textDocument/references](01KVZKK1BVQNHFX3V6W7W80YTD-show-backlinks-through-textdocumentreferences.md),
  same backlink computation, with the language server caching the scan.
