# 18. CLI command surface

Date: 2026-06-24

## Status

Accepted

## Context

The decisions so far imply a set of commands. They need concrete verbs and a
clear primary entry point, built on clap.

## Decision

clap-based subcommands. v1 surface:

- `init [path]` — initialize a vault (`all-notes/`, `.ntropy/`, default
  template and config).
- `new <title>` — create a note from the template and open it; `--no-edit` /
  `--print` creates and prints the path only.
- `search [query]` — the single browse/filter/full-text entry point. The query
  is a DSL expression (ADR 0012) and is optional (omitted = all notes). On a
  TTY it launches the interactive picker and opens the selection in `$EDITOR`;
  piped or with `-n`/`--non-interactive` it prints plainly. `list` is a visible
  alias for it; there is no distinct list command.
- `edit <id|query>` — open a specific note directly, bypassing the picker when
  unambiguous. The selector is a full 26-char ULID (resolved to that id) or
  otherwise a DSL query.
- `delete <id|query>` — remove a note and refresh views; `--force`/`-f` skips
  the confirmation prompt. Same selector rule as `edit`.
- `reconcile` — realign filenames and rebuild views.
- `view list|add|remove` — manage per-vault materialized view definitions.
  There is no `view edit` (editing is remove + add).
- `tags` — list distinct full tags with note counts.

`init` is idempotent and only writes the global default vault when passed
`--set-default`. Its target is the positional path or, when that is omitted, the
global `--vault`; passing both is an error, and with neither it scaffolds the
current directory.

`search` and `new` take their free text as the joined trailing arguments
(`ntropy search tag:work and status:done`, `ntropy new My great note`). A bare
`ntropy` with no subcommand prints help.

## Consequences

- One verb (`search`) covers filtering and full-text via the DSL, so there is
  no list/search split.
- View definitions are managed through the CLI, not only by hand-editing the
  per-vault config.
- `init` is required to bootstrap a vault.
