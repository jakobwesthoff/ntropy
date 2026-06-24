# JSON output (post-v1)

Deferred from v1 per ADR 0014. v1 output is decorated on a TTY and a
tab-separated `id<TAB>title<TAB>path` table when piped (ADR 0025).

## To decide later

- A `--json` flag for structured machine output.
- Shape: NDJSON (one note per line) vs a JSON array.
- Which fields to include (id, title, path, tags, frontmatter, derived
  created/modified).
