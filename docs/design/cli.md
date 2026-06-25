# CLI reference (v1)

The command surface and its behavior. Consolidates
[ADR 0014](../adr/0014-interactive-by-default-cli-with-auto-output-mode.md),
[ADR 0015](../adr/0015-editor-integration-and-new-note-flow.md),
[ADR 0016](../adr/0016-configuration-format-location-and-vault-resolution.md),
[ADR 0018](../adr/0018-cli-command-surface.md), and the query DSL in
[query-and-search.md](query-and-search.md).

## Global behavior

- **Vault resolution:** `--vault <path>` > `$NTROPY_VAULT` > cwd walk-up
  (nearest ancestor with either a `.ntropy-vault` pointer file or a `.ntropy/`
  dir; the pointer wins in the same dir) > global config default vault. See
  [ADR 0026](../adr/0026-project-local-vault-pointer-file.md).
- **Interactivity:** interactive on a TTY, non-interactive when piped.
  `--non-interactive` / `-n` forces non-interactive.
- **Output:** decorated for a TTY; when piped/`-n`, a tab-separated table of
  `id<TAB>date<TAB>title<TAB>tags<TAB>path`, one note per line, no header
  (awk/cut-friendly; tags comma-joined). No JSON in v1. Where a note is named to
  a human (delete prompts/confirmations, ambiguous-match lists) it is shown as
  the reference `date  title  [tags]  (id)`.
- **Ordering:** results are newest first (creation time descending) by default.
- **Editor:** `$VISUAL` then `$EDITOR`; error if neither is set.
- **Bare `ntropy`:** with no subcommand, prints help.
- **Free-text args:** `search` and `new` join their trailing arguments into one
  string (the query / the title).

## Commands

### `init [path]`

Initialize a vault at `path`: create `all-notes/`, `.ntropy/`, the templates
(`.ntropy/templates/default.md` and `today.md`), and the per-vault config seeded
with a `by-tag` view (and its `by-tag/` directory).

The target is the positional `path` or, when it is omitted, the global
`--vault`. Passing both is an error (they name the same thing two ways); with
neither, `init` scaffolds the current directory.

The default template is frontmatter `title: {{title}}` plus `tags: []` followed
by a `# {{title}}` body heading.

`init` is **idempotent**: it creates whatever is missing, leaves existing
pieces untouched, and succeeds either way. It does **not** touch the global
config unless `--set-default` is passed, which records this vault as the global
`default_vault`.

### `new <title>`

Create a note from a template (`{{title}}`, `{{id}}`, `{{date}}`, `{{slug}}`
substituted), then open it in the editor.

- `--template <name>` / `-t <name>`: use `.ntropy/templates/<name>.md` instead
  of `default.md`. A missing named template is an error; the implicit default
  falls back to the embedded template when `default.md` is absent.
- `--no-edit` / `--print`: create and print the path only (no editor).

On editor exit, ntropy reconciles the note (slug realignment, view links).

### `today`

Open today's note, the daily-note convenience. The note is identified by its
title being today's local date; if one exists it is opened, otherwise it is
created from `.ntropy/templates/today.md` (which `init` seeds and titles by
`{{date}}` with a `daily` tag). When several notes share today's date as their
title, the newest is opened. Shares `--no-edit`/`--print` with `new`. The
template must exist; a vault predating it must re-run `init`.

### `search [query]`

The single browse / filter / full-text entry point (visible alias: `list`).
`query` is an optional DSL expression; omitted means all notes.

- On a TTY: launches the interactive picker; Enter opens the selected note in
  the editor.
- Piped or `-n`: prints matching notes as plain lines.

The picker (ADR 0027) draws on the alternate screen with a prompt and an `m/n`
match counter. Type to filter; Backspace, Ctrl-W (delete word) and Ctrl-U
(clear) edit the query; Up / Ctrl-P and Down / Ctrl-N move the selection; Enter
selects; Esc / Ctrl-C aborts. The selection bar uses reverse video, so it
adapts to the terminal's theme.

Examples:

    ntropy search                       # all notes
    ntropy search tag:work
    ntropy search tag:work and not status:done   # trailing args joined
    ntropy search 'text:"deadline"'     # quote phrases for the shell
    ntropy search tag:work -n           # print, don't pick

### `edit <id|query>`

Open a specific note directly, bypassing the picker when the selector resolves
to a single note. Reconciles on exit like `new`. On an ambiguous match: a TTY
opens the picker pre-filtered to the matches; piped/`-n` errors and prints the
matches to stderr (non-zero exit).

The selector rule (shared with `delete`): an argument that is a full 26-char
ULID resolves directly to that note's id; anything else is parsed as a DSL
query.

### `delete <id|query>`

Remove a note (its canonical file) and refresh the views. The selector follows
the same id-or-query rule as `edit`. Deletion prompts for confirmation unless
`--force`/`-f` is given. An ambiguous selector behaves like `edit` (a TTY opens
the picker pre-filtered; piped/`-n` errors with a non-zero exit). In
non-interactive mode `--force` is required, since there is no prompt.

### `reconcile`

Realign filenames whose slugs drifted from their titles after out-of-band
edits, and rebuild the materialized view trees.

It prints a `Reconciling vault at <path>...` line, one `renamed <from> -> <to>`
line per realignment, and a closing summary of the notes scanned, files
renamed, views rebuilt and warnings. The summary always prints, so a no-op run
is no longer silent.

### `view list|add|remove`

Manage per-vault materialized view definitions (each pairs an output directory
with a frontmatter field; see
[vault-layout-and-views.md](vault-layout-and-views.md)). There is no `view
edit` in v1; editing a view is remove + add.

- `view list` — list configured views (`name<TAB>field`).
- `view add <name> --field <field>` — define a new view and materialize it (the
  view's directory is its name; grouping values are always normalized, ADR
  0009/0023, so there is no case flag). The name must not be reserved
  (`all-notes`, `.ntropy`) or already in use.
- `view remove <name>` — delete a view definition and its directory.

### `tags`

List all distinct full tag strings across the vault with their note counts,
sorted alphabetically (`tag<TAB>count`).

### `info`

Report the active vault and which rule resolved it (`--vault`, `$NTROPY_VAULT`,
a `.ntropy-vault` pointer, the current-directory walk-up, or the global
default), the configured global default vault, and vault statistics: note, tag,
view and template counts, the number of notes skipped with warnings, the
creation-date span, the most-used tags, and the template names. Unlike the data
commands this is a human report, printed the same way piped or on a TTY.
