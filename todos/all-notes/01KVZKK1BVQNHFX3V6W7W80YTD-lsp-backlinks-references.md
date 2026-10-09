# LSP backlinks via textDocument/references (tier 2)

Tier-2 ntropy language-server feature, deferred past the first LSP iteration
(completion + definition + documentLink + workspace/symbol).

Provide `textDocument/references` for a note: list every note whose body links
to it. Backlinks are computed on demand by scanning bodies for the note's ULID
(ADR 0028 — never stored in frontmatter). Reuses the link-extraction regex
built for link completion and the in-memory session scan cache.

## Open questions

- How to trigger references for a whole note: cursor anywhere in the document
  vs. only on the note's own identity/title.
- Whether to surface the same data through a CLI `backlinks <id>` command (see
  `01kvxwqq5vbjekr578jffved5m-linking-and-language-server.md`).
</content>
