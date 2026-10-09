---
title: "Flag links whose target note does not exist"
kind: feature
component: lsp
origin: discussion
---
# Flag links whose target note does not exist

Deferred past the first language server iteration. This is a tier 2 feature.

## Proposal

Publish `textDocument/publishDiagnostics` for links whose ULID resolves to no
note (ADR 0028 resolution). Diagnostics are computed on document open and
change, against the in-memory session scan cache, and reuse the link-extraction
regex.

## Open questions

- Should the diagnostic be a warning or a hint?
- Should a stale slug whose ULID still resolves get a weaker diagnostic? It is
  cosmetic, and `reconcile` fixes it.
