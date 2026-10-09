---
title: "Show backlinks through textDocument/references"
kind: feature
component: lsp
origin: discussion
---
# Show backlinks through textDocument/references

Deferred past the first language server iteration, which shipped completion,
definition, documentLink and workspace/symbol. This is a tier 2 feature.

## Proposal

Provide `textDocument/references` for a note, listing every note whose body
links to it. Backlinks are computed on demand by scanning bodies for the
note's ULID (ADR 0028). They are never stored in frontmatter. The feature
reuses the link-extraction regex built for link completion and the in-memory
session scan cache.

## Open questions

- Should references trigger from anywhere in the note, or only on the note's
  own identity or title?
- Should the same data also be available through a CLI `backlinks <id>`
  command?
