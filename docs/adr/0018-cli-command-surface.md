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
  piped or with `-n`/`--non-interactive` it prints plainly. There is no
  separate `list` command.
- `edit <id|query>` — open a specific note directly, bypassing the picker when
  unambiguous.
- `reconcile` — realign filenames and rebuild views.
- `view list|add|edit|remove` — CRUD over per-vault materialized view
  definitions.
- `tags` — list all tags with counts.

## Consequences

- One verb (`search`) covers filtering and full-text via the DSL, so there is
  no list/search split.
- View definitions are managed through the CLI, not only by hand-editing the
  per-vault config.
- `init` is required to bootstrap a vault.
