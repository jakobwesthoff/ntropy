# CLI reference (v1)

The command surface and its behavior. Consolidates
[ADR 0014](../adr/0014-interactive-by-default-cli-with-auto-output-mode.md),
[ADR 0015](../adr/0015-editor-integration-and-new-note-flow.md),
[ADR 0016](../adr/0016-configuration-format-location-and-vault-resolution.md),
[ADR 0018](../adr/0018-cli-command-surface.md), and the query DSL in
[query-and-search.md](query-and-search.md).

## Global behavior

- **Vault resolution:** `--vault <path>` > `$NTROPY_VAULT` > cwd walk-up to the
  nearest ancestor containing `.ntropy/` > global config default vault.
- **Interactivity:** interactive on a TTY, non-interactive when piped.
  `--non-interactive` / `-n` forces non-interactive.
- **Output:** decorated for a TTY; when piped/`-n`, a tab-separated table of
  `id<TAB>title<TAB>path`, one note per line, no header (awk/cut-friendly). No
  JSON in v1.
- **Ordering:** results are newest first (creation time descending) by default.
- **Editor:** `$VISUAL` then `$EDITOR`; error if neither is set.

## Commands

### `init [path]`

Initialize a vault at `path` (or the current directory): create `all-notes/`,
`.ntropy/`, a default template, and per-vault config.

### `new <title>`

Create a note from the default template (`{{title}}`, `{{id}}`, `{{date}}`,
`{{slug}}` substituted), then open it in the editor.

- `--no-edit` / `--print`: create and print the path only (no editor).

On editor exit, ntropy reconciles the note (slug realignment, view links).

### `search [query]`

The single browse / filter / full-text entry point. `query` is an optional DSL
expression; omitted means all notes.

- On a TTY: launches the interactive picker; Enter opens the selected note in
  the editor.
- Piped or `-n`: prints matching notes as plain lines.

Examples:

    ntropy search                       # all notes
    ntropy search tag:work
    ntropy search 'tag:work and not status:done'
    ntropy search 'text:"deadline"'
    ntropy search tag:work -n           # print, don't pick

### `edit <id|query>`

Open a specific note directly, bypassing the picker when the selector resolves
to a single note. Reconciles on exit like `new`. On an ambiguous match: a TTY
opens the picker pre-filtered to the matches; piped/`-n` errors and prints the
matches to stderr (non-zero exit).

### `reconcile`

Realign filenames whose slugs drifted from their titles after out-of-band
edits, and rebuild the materialized view trees.

### `view list|add|edit|remove`

CRUD over per-vault materialized view definitions (each pairs an output
directory with a frontmatter field; see
[vault-layout-and-views.md](vault-layout-and-views.md)).

- `view list` — list configured views.
- `view add <name> --field <field>` — define a new view (the view's directory
  is its name; grouping values are always normalized, ADR 0009/0023, so there
  is no case flag).
- `view edit <name> …` — modify a view.
- `view remove <name>` — delete a view definition and its directory.

### `tags`

List all tags across the vault with note counts.
