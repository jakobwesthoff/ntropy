# CLI link helpers (post-v1)

Out-of-band link commands for users not running the language server. Deferred
from the first linking iteration, which shipped the link format (ADR 0028) and
the LSP.

## To build

- A `link` command: picker selects a target, emits/copies link markup
  `[display](<ulid>-<slug>.md)`.
- A `backlinks <id>` command: scan note bodies for references to the ULID.

## Considerations

- Backlink performance over large vaults under the stateless model (ADR 0002):
  the `backlinks` command scans all bodies on each invocation with no persistent
  cache (unlike the LSP's in-memory session scan cache).
