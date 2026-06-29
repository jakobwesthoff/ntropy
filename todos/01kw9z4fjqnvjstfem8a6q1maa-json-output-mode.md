# Structured (JSON) output mode for machine consumers

## Why

The non-interactive tables (`print_notes`, `print_tags`, `view list`) are moving
to space-aligned, human-readable output for *all* invocations (TTY, piped, and
`-n`), superseding the tab-separated machine contract of ADR 0025. Aligned text
is not robustly parsable: a value containing whitespace (note titles always do)
breaks positional field extraction, and the delimiter can no longer be
guaranteed absent from the data. See the column-alignment todo
`01kw9dnybzecacbm75wqaaz02e` for the full reasoning and the demonstration that
neither space-padding nor multi-tab padding survives `awk`/`cut`.

Dropping the tab contract therefore leaves machine consumers without a reliable
parse path. JSON (or another structured format) is the replacement contract: a
human reads the aligned table, a script asks for JSON.

## Scope

A global flag (e.g. `--json`, or a broader `--format <table|json>`) that switches
the table-producing commands to structured output:

- `search` / `list` (the note table): emit an array of note objects
  (`id`, `date`, `title`, `tags` as a real array, `path`).
- `tags`: emit `[{ "tag": ..., "count": ... }]`.
- `view list`: emit `[{ "name": ..., "field": ... }]`.

Open questions to settle when picking this up:

- Flag shape: boolean `--json` vs. an extensible `--format` enum. `--format`
  leaves room for future formats (NDJSON, CSV) but is more surface area now.
- Does `info` participate, or stay a human-only report? (It is explicitly *not*
  a machine table today, `output.rs:83`.)
- NDJSON (one object per line) vs. a single JSON array. NDJSON streams and
  composes with `jq -c`/line tools; a single array is simpler and matches the
  "one document" mental model. Note ordering must stay newest-first either way
  (ADR 0025 ordering decision still stands).
- Where the format decision is threaded. The aligned/JSON choice is the same
  dispatch-layer concern as the current `TableStyle`; a single renderer entry
  point in `output.rs` taking the format keeps all three tables consistent.
- Error/exit-code behavior is unchanged (no-match still exits non-zero, warnings
  still go to stderr so stdout stays valid JSON).

## Related

- `src/bin/ntropy/run/output.rs` — `print_notes`, `print_tags`
- `src/bin/ntropy/run/mod.rs:316` — inlined `view list` table
- `docs/adr/0025-plain-output-format-ordering-and-edit-ambiguity.md` — the
  superseded tab contract; this todo is its structured-output successor
- `01kw9dnybzecacbm75wqaaz02e-non-interactive-table-column-alignment.md` — the
  alignment change that motivates this
