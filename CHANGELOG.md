# Changelog

All notable changes to ntropy are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.0.0 - 2026-06-26

First stable release: the user-facing interface is polished and initial
language-server support is added.

### Fixed

- `init` now honors the global `--vault` flag as the target when no positional
  path is given, instead of silently scaffolding the current directory. Passing
  both a path and `--vault` is rejected as a conflict.

### Added

- `info` command: reports the active vault and how it resolved, the global
  default vault, and vault statistics (note/tag/view/template counts, warnings,
  creation-date span, top tags, and template names).
- `today` command: opens today's note (titled by the date), creating it from the
  seeded `today` template on first use each day and reopening it afterward. `init`
  now also seeds `.ntropy/templates/today.md`.
- `new --template <name>` / `-t <name>` selects a template from
  `.ntropy/templates/<name>.md`; a missing named template is an error. Without
  the flag, `default.md` is used as before. See the README Templates section.
- `list` is now a visible alias for `search`.
- `reconcile` now prints a start line and a closing summary (notes scanned,
  files renamed, links relinked, views rebuilt, warnings). The summary always
  prints, so a no-op run is no longer silent.
- Inter-note links: a standard Markdown link whose target is the note filename,
  `[text](<ulid>-<slug>.md)`, is recognized by its leading 26-character ULID.
  `reconcile` refreshes stale link slugs to a note's current filename, keeping
  links resolvable and clickable after a rename. Links inside fenced or inline
  code are left untouched.
- Language server (`ntropy lsp`): an editor-agnostic LSP server over stdin/stdout
  that completes inter-note links (type `[`, pick a note by fuzzy-matching its
  title and tags) and frontmatter tags (flow and block forms, hierarchy-aware),
  and provides go-to-definition, document links, and workspace-symbol search
  across notes. It resolves a vault per open document and keeps an in-memory
  session cache refreshed by editor file-watch events. See
  [docs/design/language-server.md](docs/design/language-server.md).

### Changed

- Plain tab-separated tables (`search -n`, `tags`, `view list`) now start with an
  uppercase column header (docker-style). Strip it with `tail -n +2` if needed.
- The interactive fuzzy picker is now rendered in-house over `nucleo` and
  `crossterm` instead of `nucleo-picker`. It is bottom-anchored: the query
  prompt is framed by a blue divider line above and below it, with a dimmed
  stats line beneath (under the query text) showing the cursor's rank within the
  matches and the match/total counts, and the result list grows upward with the
  best match nearest the prompt. Rows are an aligned title/date/tags grid
  (widths measured in Unicode display columns) with the note's ULID shown dimmed
  and never matched. Matched characters are highlighted in yellow and the
  selected row in cyan with a `▌` bar, all from the terminal's own ANSI palette
  so the picker adapts to its light/dark theme. Type to filter; Ctrl-W (delete
  word), Ctrl-U (clear), and Up/Ctrl-P (toward worse matches) / Down/Ctrl-N
  (toward the best) navigation (ADR 0027).
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
