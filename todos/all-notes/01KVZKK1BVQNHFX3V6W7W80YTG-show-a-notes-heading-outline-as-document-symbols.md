---
title: "Show a note's heading outline as document symbols"
kind: feature
component: lsp
origin: discussion
---
# Show a note's heading outline as document symbols

Deferred past the first language server iteration. This is a tier 2 feature.

## Proposal

Provide a heading outline for the open note through `textDocument/documentSymbol`.
This enables structure navigation and breadcrumbs in the editor. It needs
light Markdown heading scanning of the buffer (ATX `#` headings), not a full
parser.
