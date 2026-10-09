---
title: "Show link target previews on hover"
kind: feature
component: lsp
origin: discussion
---
# Show link target previews on hover

Deferred past the first language server iteration. This is a tier 2 feature.

## Proposal

On hovering an ntropy link, resolve its ULID (ADR 0028) and show the target
note's title and a short body excerpt, without opening the note. The feature
is served by `textDocument/hover` and reuses link resolution and the in-memory
session scan cache.

## Open questions

- How is the excerpt extracted: the first paragraph after the frontmatter, or
  the first N lines?
