# Site: backlinks on note pages

Deferred from the note page of the site (ADR 0054,
`docs/design/site-export.md`). Decision (user, 2026-09-11): not in the first
implementation.

Nothing in the library computes backlinks today. ADR 0028 states the
intent (compute on demand by scanning bodies for the target ULID); the LSP
and CLI surfaces for it are open todos
(`01kvzkk1bvqnhfx3v6w7w80ytd-lsp-backlinks-references.md`,
`01kw4bdqkqfn8ep41cet8b1avz-cli-link-helpers.md`).

## What was discussed

- A "linked from" section per note page, computed at export from the
  resolved link tables of all exported notes. The export already builds
  every note's link table, so the inverse index is a fold over them.

## To decide later

- Whether the computation lives in the library so the LSP and CLI todos
  reuse it.
- Whether links from notes outside the exported set (ADR 0046's query
  filter) count.
