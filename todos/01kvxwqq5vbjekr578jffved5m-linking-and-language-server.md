# Inter-note linking and a language server (post-v1)

Deferred from v1. ntropy does not control the editor buffer, so link insertion
and navigation are out-of-band; nothing in v1 processes links, which is why v1
ships without a Markdown parser.

## Scope to design later

- **Link format:** standard Markdown link with the target note's ULID as the
  target: `[display](<ulid>)`. Resolution via the ULID glob (ADR 0004).
- **Link commands:** an out-of-band `link` helper (picker → emit/copy link
  markup), `backlinks <id>` (scan bodies for references), and follow/resolve a
  `[[...]]`/`(ulid)` target to a note path.
- **Language server:** an LSP that understands ntropy notes and provides
  completion for links and tags (and likely frontmatter fields) inside the
  editor. This is the primary intended way to author links ergonomically,
  given ntropy cannot inject into an external editor buffer.

## Open questions for that work

- Whether link scanning needs a real Markdown parser (e.g. `pulldown-cmark`) or
  an LSP-oriented incremental parser (e.g. tree-sitter) instead of regex, to
  ignore links in code blocks and parse robustly.
- Backlink performance over large vaults under the stateless model (ADR 0002).
