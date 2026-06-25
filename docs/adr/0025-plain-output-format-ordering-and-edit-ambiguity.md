# 25. Plain output format, ordering and edit ambiguity

Date: 2026-06-24

## Status

Accepted

## Context

The non-interactive output format, default result ordering, and the behavior of
`edit` on an ambiguous selector were left open in the CLI design.

## Decision

- Non-interactive output is a tab-separated table, one note per line:
  `id<TAB>date<TAB>title<TAB>tags<TAB>path` (tags comma-joined within their
  field). It is led by an uppercase column header (`ID DATE TITLE TAGS PATH`)
  so the output is self-describing; `awk`/`cut` still split each row, and
  `tail -n +2` strips the header. All plain tab tables follow this rule: `tags`
  prints `TAG<TAB>COUNT` and `view list` prints `NAME<TAB>FIELD`.
- Default result ordering is newest first (ULID / creation time descending). A
  `--sort` flag is left for later.
- `edit <query>` on an ambiguous match: on a TTY, open the picker pre-filtered
  to the matches; when non-interactive/piped, error and print the matches to
  stderr with a non-zero exit. Each match prints as the shared human reference
  `date  title  [tags]  (id)`, the same representation used in delete prompts
  and confirmations.

## Consequences

- Pipelines get id, date, title, tags, and path in one pass without extra
  lookups; the header row is dropped with `tail -n +2` when not wanted.
- Recency-first matches the common note-taking expectation.
- Ambiguous `edit` is smooth interactively and safe (no silent wrong-note open)
  in scripts.
- One human note reference is used wherever a note is named to a person
  (ambiguous lists, delete prompts and confirmations).
