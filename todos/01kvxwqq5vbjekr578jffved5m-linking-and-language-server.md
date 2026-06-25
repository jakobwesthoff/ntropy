# Inter-note linking and a language server

The link format and the core language server are now being designed for
implementation. This todo tracks the parts that remain deferred.

## Decided

- **Link format:** standard Markdown link to the target's current filename,
  `[display](<ulid>-<slug>.md)`, resolved by ULID glob. See ADR 0028.
- **LSP stack:** `lsp-server` + `lsp-types` (synchronous), validated by building.
- **First LSP iteration scope:** completion (links + frontmatter tags),
  `textDocument/definition`, `textDocument/documentLink`, `workspace/symbol`.
- **Backlinks:** computed on demand by scanning for the ULID; never stored
  (ADR 0028).

## Still deferred

- **CLI link helpers** (for users not running the LSP): an out-of-band `link`
  command (picker → emit/copy link markup) and a `backlinks <id>` command (scan
  bodies for references).
- **Tier-2 LSP features**, one todo each:
  - `01kvzkk1bvqnhfx3v6w7w80ytd-lsp-backlinks-references.md`
  - `01kvzkk1bvqnhfx3v6w7w80yte-lsp-hover-preview.md`
  - `01kvzkk1bvqnhfx3v6w7w80ytf-lsp-dangling-link-diagnostics.md`
  - `01kvzkk1bvqnhfx3v6w7w80ytg-lsp-document-symbol-outline.md`

## Considerations

- Backlink performance over large vaults under the stateless model (ADR 0002),
  mitigated in the LSP by an in-memory session scan cache.
</content>
