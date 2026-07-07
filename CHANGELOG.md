# Changelog

All notable changes to ntropy are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v1.5.0 - 2026-07-07

### Added

- `init` now seeds a `README.md` in the vault root that identifies the
  directory as an ntropy vault, links to <https://ntropy.westhoffswelt.de>,
  and shows how to install the CLI (`cargo install ntropy`), so anyone who
  discovers a vault knows how to access it. Like the templates, it is written
  only when absent: a re-init restores a deleted README but never overwrites
  an edited one. `README.md` is now a reserved name, so a view cannot clobber
  it.

## v1.4.0 - 2026-07-06

### Added

- An agent skill under `skills/ntropy/` that teaches LLM coding agents how to
  drive ntropy: a `SKILL.md` with the vault model and the non-interactive
  ground rules (always `-n`, `--no-edit` on `new`/`today`, `reconcile` after
  direct edits), plus reference docs on writing notes, querying, vaults, and
  views. Install it with `npx skills add jakobwesthoff/ntropy` or by copying
  the directory into an agent's skills folder; see the README's "Agent skill"
  section.

### Fixed

- Piping ntropy's output into a reader that exits early (e.g. `ntropy info |
  head -2`) no longer panics with `failed printing to stdout: Broken pipe (os
  error 32)`. ntropy now exits quietly on a closed stdout pipe, like
  conventional Unix tools (status 141).
- Titles with YAML-special characters (`Q3: Planning kickoff`, `[draft]
  roadmap`, `#hashtag first`) previously made `new` fail. Frontmatter
  placeholder substitution is now YAML-aware, quoting or escaping a
  substituted value only when its surrounding YAML needs it, so such titles
  work.
- A `new` whose template rendered an invalid note (e.g. one missing a `title`
  field) previously left the malformed file behind in `all-notes/`; every
  later command then warned about it until it was cleaned up by hand. The
  rendered note is now validated before anything is written, so a failed
  `new` leaves nothing behind.

## v1.3.0 - 2026-06-29

### Changed

- The plain tables (`search`/`list`, `tags`, `view list`) now render with
  space-aligned columns for every invocation, including piped and `-n` output.
  Columns are padded to their widest cell in Unicode display width with the last
  column left unpadded, so values that overflow a tab stop no longer push the
  following columns out of line. The tab-separated `awk`/`cut` positional format
  is retired (ADR 0033); `tail -n +2` still drops the header. Structured (JSON)
  output for machine consumers is planned.

### Fixed

- The interactive picker now fuzzy-searches the full note content. Titles and
  tag lists are clipped to fit their columns, but the matcher previously only
  saw the clipped text, so a long title's tail or a tag past the visible cap was
  unfindable. Matching now runs over the untruncated title, tags and date, while
  the columns stay width-capped; a match that lands in clipped-away text ranks
  the note without painting a stray highlight.

## 1.2.0 - 2026-06-27

### Added

- ntropy maintains a root `.gitignore` listing the derived materialized view
  directories, so committing a vault no longer tracks them. The entries stay in
  sync with the configured views through `init`, `reconcile`, and `view
  add`/`view remove`; lines you add to the file yourself are never touched.

### Changed

- Materialized views now refresh incrementally. After a mutation (and during
  `reconcile`), each view is diffed against its on-disk tree and only the links
  that actually changed are touched, instead of tearing down and regenerating
  every view tree from scratch. Unchanged links keep their identity, and a
  mutation's filesystem cost is proportional to what changed rather than to the
  whole vault. On a 3000-note vault with two views (Apple M1), this cuts a
  mutation or `reconcile` from roughly 820 ms to 135–150 ms (about 5–6×), with
  the saved time being almost entirely filesystem syscalls. `reconcile`'s
  summary now reads `synced N views` rather than `rebuilt N views`.
- `view remove` no longer deletes the view's directory. ntropy never deletes a
  directory: it prunes the view's `.gitignore` entry and leaves the now-stale
  directory in place, reporting it so you can delete it yourself. `reconcile`
  likewise prunes entries for views removed from config without touching their
  directories.

## 1.1.0 - 2026-06-26

### Fixed

- The interactive picker no longer panics on Ctrl-W when the query contains
  multi-byte whitespace (e.g. a non-breaking space): word deletion now advances
  by whole characters instead of bytes.
- Block-form tag completion no longer corrupts a tag containing a hyphen
  (`area/work-home`): the list-item dash is located by structure rather than by
  the last hyphen on the line, so accepting a suggestion replaces the whole tag.
- The picker now restores raw mode even when entering the alternate screen
  fails on startup, instead of leaving the shell without echo for the rest of
  the session.
- Link completion no longer drops a space from a display title that literally
  contains `) $0`; the snippet placeholder cleanup is confined to the snippet
  branch.
- Vault walk-up now reports a directory that looks like a vault but cannot be
  canonicalized as an error, instead of silently treating it as "no vault found"
  and falling through to the global default.
- The language server no longer points one character too far when a client
  sends an out-of-spec position inside a surrogate pair; it clamps to the start
  of the affected character.

### Changed

- Link completion now cooperates with editors that auto-close brackets: when the
  closing `]` was already inserted, accepting a completion overwrites it instead
  of leaving a duplicate.
- `reconcile` resolves link targets through an index rather than a linear scan
  per link, so refreshing links in large, well-linked vaults is markedly faster.

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
- `edit` is now a hidden alias of `search` rather than a separate command
  (ADR 0031). `search`/`list` accepts a full ULID or a query and is the single
  open entry point: on a TTY a single match opens directly in the editor and
  several open the picker, while piped/`-n` prints the plain table without ever
  opening an editor. A selector or listing that matches nothing now exits
  non-zero with the message on stderr.
- Full-text search (`text:` and bare terms) now uses the `regex` crate in place
  of the embedded ripgrep libraries (`grep-searcher`/`grep-regex`). Smart-case
  and matching are unchanged, except a pattern that explicitly spans a newline
  now matches across lines instead of being confined to one (ADR 0030).

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
