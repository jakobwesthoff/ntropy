# 25. Plain output format, ordering and edit ambiguity

Date: 2026-06-24

## Status

Accepted

## Context

The non-interactive output format, default result ordering, and the behavior of
`edit` on an ambiguous selector were left open in the CLI design.

## Decision

- Non-interactive output is a tab-separated table, one note per line:
  `id<TAB>title<TAB>path`. No header or decoration, so `awk`/`cut` can split
  it directly.
- Default result ordering is newest first (ULID / creation time descending). A
  `--sort` flag is left for later.
- `edit <query>` on an ambiguous match: on a TTY, open the picker pre-filtered
  to the matches; when non-interactive/piped, error and print the matches to
  stderr with a non-zero exit.

## Consequences

- Pipelines get id, title, and path in one pass without extra lookups.
- Recency-first matches the common note-taking expectation.
- Ambiguous `edit` is smooth interactively and safe (no silent wrong-note open)
  in scripts.
