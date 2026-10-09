---
title: "Add structured JSON output for machine consumers"
kind: feature
component: cli
origin: discussion
---
# Add structured JSON output for machine consumers

Deferred during the v1 design (ADR 0014). v1 output is decorated on a TTY, and
piped output is a tab-separated `id<TAB>title<TAB>path` table (ADR 0025). The
non-interactive tables (`print_notes`, `print_tags`, `view list`) are moving to
space-aligned, human-readable output for all invocations, TTY, piped and `-n`
alike. That supersedes the tab-separated machine contract of ADR 0025.

## Goal

Aligned text is not robustly parsable. A value containing whitespace breaks
positional field extraction, and note titles always contain whitespace. The
delimiter can no longer be guaranteed absent from the data. Padding with spaces
or with multiple tabs does not survive `awk` or `cut`.

Dropping the tab contract therefore leaves machine consumers without a reliable
parse path. JSON, or another structured format, becomes the replacement
contract: a human reads the aligned table, and a script asks for JSON.

## Proposal

A global flag, such as `--json` or a broader `--format <table|json>`, switches
the table-producing commands to structured output:

- `search` and `list` (the note table) emit an array of note objects with `id`,
  `date`, `title`, `tags` as a real array, and `path`.
- `tags` emits `[{ "tag": ..., "count": ... }]`.
- `view list` emits `[{ "name": ..., "field": ... }]`.

Error and exit-code behaviour stays unchanged. No match still exits non-zero, and
warnings still go to stderr, so stdout stays valid JSON.

## Open questions

- Should the flag be a boolean `--json`, or an extensible `--format` enum? The
  enum leaves room for NDJSON or CSV later, but adds surface area now.
- Does `info` take part, or does it stay a human-only report? It is explicitly
  not a machine table today (`output.rs`, around line 83).
- NDJSON (one object per line) or a single JSON array? NDJSON streams and
  composes with `jq -c` and line tools. A single array is simpler and matches
  the one-document model. Note ordering stays newest-first either way (the
  ordering decision of ADR 0025 still stands).
- Which fields go into each note object? The note table lists id, date, title,
  tags and path. Should frontmatter and derived created and modified dates be
  included too?
- Where is the format decided and threaded through? The aligned or JSON choice
  is the same dispatch-layer concern as the current `TableStyle`. A single
  renderer entry point in `output.rs` that takes the format would keep all three
  tables consistent.

## References

- `src/bin/ntropy/run/output.rs`: `print_notes`, `print_tags`.
- `src/bin/ntropy/run/mod.rs`: the inlined `view list` table, around line 316.
- ADR 0025: the tab contract that this supersedes.
