# 31. Merge edit into search

Date: 2026-06-26

## Status

Accepted

Supersedes the edit-ambiguity decision of
[ADR 0025](0025-plain-output-format-ordering-and-edit-ambiguity.md) and the
`edit` command of [ADR 0018](0018-cli-command-surface.md).

## Context

`edit` and `search` shared nearly all of their machinery: both resolved an
optional selector, both listed matches, both opened the picker and the editor
through the same helpers. They diverged in only three ways: `edit` resolved a
full ULID directly, `edit` opened a single match without the picker, and `edit`
enforced a strict "exactly one or fail" contract non-interactively (erroring on
an ambiguous or absent selector while `search` always succeeded). The split put
two verbs over one engine.

## Decision

One command, `search` (visible alias `list`, hidden alias `edit`). The selector
is optional: omitted lists the whole vault; otherwise it resolves a full
26-character ULID directly, or runs as a DSL query (the id-or-query rule of
ADR 0025).

- On a TTY a single match opens directly in the editor; several open the picker
  pre-filtered to them, and the selection opens and reconciles.
- Non-interactive, the matching notes print as the plain table (one row for a
  single match, the full table for several). The editor never opens without a
  TTY, matching `new` and `today` (ADR 0015).
- A no-match, in any mode, prints `No notes matched your search criteria.` to
  stderr and exits non-zero. This is uniform: an empty-vault listing exits
  non-zero too.

`delete` keeps the strict id-or-query resolution and the ambiguous-selector
error (ADR 0025); it must resolve to exactly one note.

## Consequences

- `edit <id|query>` keeps working as a hidden alias, byte-for-byte identical to
  `search`; the dedicated command is gone.
- The "non-interactive never opens an editor" rule is now uniform across `new`,
  `today` and `search`.
- A no-match selector or empty result is a non-zero exit, so `search <x> && …`
  branches correctly; the message goes to stderr so stdout stays clean for
  pipelines.
- `search` no longer treats an empty result as success; scripts relying on
  exit 0 for "no matches" must branch on the exit code instead.
