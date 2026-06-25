# Changelog

All notable changes to ntropy are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- `init` now honors the global `--vault` flag as the target when no positional
  path is given, instead of silently scaffolding the current directory. Passing
  both a path and `--vault` is rejected as a conflict.

### Added

- `list` is now a visible alias for `search`.
- `reconcile` now prints a start line and a closing summary (notes scanned,
  files renamed, views rebuilt, warnings). The summary always prints, so a
  no-op run is no longer silent.

### Changed

- The interactive fuzzy picker is now rendered in-house over `nucleo` and
  `crossterm` instead of `nucleo-picker`. The selection bar uses reverse video
  so it adapts to the terminal's light/dark theme, matched characters are
  highlighted, and the picker supports Ctrl-W (delete word), Ctrl-U (clear),
  and Ctrl-P/Ctrl-N navigation (ADR 0027). Rows now also show the note's ULID
  dimmed, without matching against it.
- A single note reference (`date  title  [tags]  (id)`) is now used everywhere a
  note is named to a person: delete prompts and confirmations and the
  ambiguous-match list. The plain `search -n` table gained `date` and `tags`
  columns: `id<TAB>date<TAB>title<TAB>tags<TAB>path` (tags comma-joined). This
  changes the previous `id<TAB>title<TAB>path` format.

## [0.9.0] - 2026-06-25

Initial release: a working, Unix-only (macOS, Linux) v1 of the ntropy CLI.

### Added

- Flat single-vault storage with canonical notes as
  `all-notes/<ulid>-<slug>.md`; identity is carried by the filename ULID and
  never stored in frontmatter.
- Permissive YAML frontmatter with recognized `title` (required) and `tags`,
  plus arbitrary preserved fields, and slash-separated hierarchical tags with
  German-aware slug/tag normalization.
- Stateless parallel scan of `all-notes/` that warns and skips malformed or
  badly-named notes; `--strict` promotes those warnings to errors.
- Query DSL (precedence `not` > `and` > `or`, parentheses) with `tag:` segment
  sub-path matching, `field:` equality and list membership, and regex `text:`
  full-text search with smart-case via the embedded ripgrep libraries.
- Materialized symlink views: group by any frontmatter field, list fan-out,
  `/`-nesting, normalized grouping values, `<date>-<slug>.md` leaves with
  trailing-ULID collision disambiguation, and relocatable relative link targets.
- `reconcile` to realign drifted filenames and rebuild views; views are also
  refreshed after every mutation.
- Note templates with `{{title}}`/`{{id}}`/`{{date}}`/`{{slug}}` substitution
  and a default template.
- Two-tier TOML configuration (global default vault, per-vault view
  definitions) and vault resolution order `--vault` > `$NTROPY_VAULT` > cwd
  walk-up (honoring a `.ntropy-vault` pointer) > global default.
- Commands: `init` (idempotent, `--set-default`), `new`
  (`--no-edit`/`--print`), `search`, `edit`, `delete` (`--force`),
  `reconcile`, `view list|add|remove`, and `tags`; with global `--vault`,
  `-n`/`--non-interactive`, and `--strict`.
- Interactive fuzzy picker on a TTY and `$VISUAL`/`$EDITOR` integration, with a
  plain newest-first `id<TAB>title<TAB>path` table when piped or run with `-n`.
- Derived dates rendered in the system-local timezone.

[0.9.0]: https://github.com/jakobwesthoff/ntropy/releases/tag/v0.9.0
