# CLI reference (v1)

The command surface and its behavior. Consolidates
[ADR 0014](../adr/0014-interactive-by-default-cli-with-auto-output-mode.md),
[ADR 0015](../adr/0015-editor-integration-and-new-note-flow.md),
[ADR 0016](../adr/0016-configuration-format-location-and-vault-resolution.md),
[ADR 0018](../adr/0018-cli-command-surface.md),
[ADR 0031](../adr/0031-merge-edit-into-search.md),
[ADR 0036](../adr/0036-interactivity-keyed-to-the-controlling-terminal.md),
[ADR 0037](../adr/0037-render-command-surface.md),
and the query DSL in [query-and-search.md](query-and-search.md).

## Global behavior

- **Vault resolution:** `--vault <path>` > `$NTROPY_VAULT` > cwd walk-up
  (nearest ancestor with either a `.ntropy-vault` pointer file or a `.ntropy/`
  dir; the pointer wins in the same dir) > global config default vault. See
  [ADR 0026](../adr/0026-project-local-vault-pointer-file.md).
- **Interactivity:** interactive whenever a controlling terminal (`/dev/tty`)
  is available; `--non-interactive` / `-n` forces non-interactive, and
  environments without a controlling terminal (cron, CI) are non-interactive
  automatically. Redirecting stdout does not demote to non-interactive: stdout
  is purely a data channel, and the picker, the delete confirmation, and the
  editor all talk to the controlling terminal directly (ADR 0036).
- **Output:** the plain tables are space-aligned for every
  invocation: the note table is `id date title tags path`,
  one note per line, led by an uppercase column header, columns padded to their
  widest cell in Unicode display width with the last column unpadded (tags
  comma-joined; `tail -n +2` drops the header). All plain tables carry a header.
  The tab-separated `awk`/`cut` positional format is retired (ADR 0033);
  structured JSON output for machine consumers is planned. Where a note is named
  to a human (delete prompts/confirmations, ambiguous-match lists) it is shown
  as the reference `date  title  [tags]  (id)`.
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
- `--print` / `-p`: create and print the path only (no editor). `--no-edit`
  is a hidden alias (ADR 0035).

On editor exit, ntropy reconciles the note (slug realignment, view links).

### `today`

Open today's note, the daily-note convenience. The note is identified by its
title being today's local date; if one exists it is opened, otherwise it is
created from `.ntropy/templates/today.md` (which `init` seeds and titles by
`{{date}}` with a `daily` tag). When several notes share today's date as their
title, the newest is opened. Shares `--print`/`-p` with `new`. The
template must exist; a vault predating it must re-run `init`.

### `search [id|query]`

The single browse / filter / full-text / open entry point (visible alias:
`list`; hidden alias: `edit`). The selector is optional: omitted means all
notes; a full 26-char ULID resolves directly to that note; anything else is a
DSL expression (the id-or-query rule shared with `delete`).

- Interactive: a single match opens directly in the editor; several launch the
  interactive picker pre-filtered to them, and Enter opens the selection. The
  note reconciles on editor exit, like `new`. Nothing is written to stdout, so
  a redirected stdout receives nothing without `--print` (ADR 0036).
- `--print` / `-p`: print paths to stdout instead of opening the editor.
  Interactively the picker chooses the one path (`search -p | pbcopy`); a
  cancelled picker exits non-zero. With `-n` (or no controlling terminal)
  every match prints, one path per line, newest first. `--no-edit` is a
  hidden alias (ADR 0035).
- `-n` (or no controlling terminal): prints the matching notes as the plain
  table (one row for a single match, the full table for several). The editor
  never opens, mirroring `new`/`today` (ADR 0015).
- No match, in any mode: prints `No notes matched your search criteria.` to
  stderr and exits non-zero. An empty-vault listing exits non-zero too
  (ADR 0031).

The picker (ADR 0027) draws on the alternate screen, bottom-anchored: the prompt
is framed by a blue divider line above and below it, with a dimmed stats line
beneath the lower divider (indented under the query text, showing the cursor's
rank within the matches, the match count and the total), and the result list
grows upward above the top divider with the best match nearest the prompt. Rows are an aligned
title/date/tags grid (widths in Unicode display columns) with the note's ULID
trailing dimmed and never matched. Type to filter; Backspace, Ctrl-W (delete
word) and Ctrl-U (clear) edit the query; Up / Ctrl-P move toward worse matches
and Down / Ctrl-N toward the best; Enter selects; Esc / Ctrl-C aborts. Matched
characters are yellow and the selected row cyan with a `▌` bar, all from the
terminal's own ANSI palette so the picker adapts to its theme. Each frame is
delivered to the terminal as one buffered write bracketed by a synchronized
update (DEC mode 2026, applied atomically where supported), erases per line
instead of clearing the screen, and keeps the cursor hidden until it is
parked at the prompt, so redraws neither flicker nor trigger cursor-trail
animations.

Examples:

    ntropy search                       # all notes
    ntropy search tag:work
    ntropy search 01ARZ3NDEKTSV4RRFFQ69G5FAV    # open a note by id
    ntropy search tag:work and not status:done   # trailing args joined
    ntropy search 'text:"deadline"'     # quote phrases for the shell
    ntropy search tag:work -n           # print, don't pick
    ntropy edit 01ARZ3NDEKTSV4RRFFQ69G5FAV      # `edit` is a hidden alias

### `delete <id|query>`

Remove a note (its canonical file) and refresh the views. The selector follows
the same id-or-query rule as `search`. Deletion prompts for confirmation unless
`--force`/`-f` is given; the prompt goes through the controlling terminal, so
redirected streams can neither swallow the question nor feed the answer
(ADR 0036). Unlike `search`, `delete` must resolve to exactly one note: an
ambiguous selector opens the picker pre-filtered interactively, and errors
with a non-zero exit under `-n` (ADR 0025). In non-interactive mode `--force`
is required, since there is no prompt.

### `render <id|query>`

Turn one note into a document artifact. v1 produces a PDF through pandoc with
typst as the PDF engine, so both tools must be installed and on `PATH`; an
engine whose tools are missing is an error naming what to install, never a
silent fall-back. The selector follows the same id-or-query rule as `search`.
Like `delete`, `render` must resolve to exactly one note: an ambiguous selector
opens the picker pre-filtered interactively, and errors with the candidate list
under `-n` (ADR 0025/0036). A cancelled picker exits non-zero under `-p`, so
`open "$(ntropy render -p ...)"` branches correctly, and is a successful no-op
without it, like `delete`.

- `--to <format>` names the output format and defaults to `pdf`.
- `--engine <name>` overrides the format's default engine; v1 has only the
  `pandoc` engine, so the flag accepts that single value and exists so
  invocations written today keep working when other engines arrive.
- `--output <path>` / `-o` names the artifact; the default is `./<slug>.pdf` in
  the current directory, from the slug component of the note's filename. An
  existing file at the target is overwritten.
- `--print` / `-p` prints the artifact's path to stdout as one line on success;
  without it stdout stays silent and the file is the outcome (ADR 0036).
- Scan warnings print to stderr and fail the command under `--strict`, matching
  `search`.

`render` is read-only: no filename realignment and no view refresh.

Examples:

    ntropy render 01ARZ3NDEKTSV4RRFFQ69G5FAV        # render one note by id
    ntropy render tag:work -n                        # error on an ambiguous selector
    open "$(ntropy render -p 'text:"quarterly"')"    # render, then open the PDF
    ntropy render 01ARZ3NDEKTSV4RRFFQ69G5FAV -o report.pdf

### `reconcile`

Realign filenames whose slugs drifted from their titles after out-of-band
edits, refresh inter-note link targets to the current filenames, sync the
materialized view trees, and sync the root `.gitignore` to the configured
views (ADR 0032).

After renaming, it rewrites stale link targets in note bodies so links keep
resolving and stay clickable in plain Markdown viewers (ADR 0028); links inside
fenced or inline code are left untouched.

The `.gitignore` sync adds an entry for every configured view and prunes the
entries of views removed from config, leaving user-authored lines untouched.
ntropy never deletes a directory, so a removed view's directory stays on disk;
because its ignore entry is gone it becomes visible to git, which the run
reports so the user can delete it.

It prints a `Reconciling vault at <path>...` line, one `renamed <from> -> <to>`
line per realignment, one `relinked <from> -> <to> in <file>` line per refreshed
link, one `ignored <entry>` line per added ignore and a `stopped ignoring ...`
line per pruned one, and a closing summary of the notes scanned, files renamed,
links relinked, views synced, ignore entries added and removed, and warnings.
The summary always prints, so a no-op run is no longer silent.

### `view list|add|remove`

Manage per-vault materialized view definitions (each pairs an output directory
with a frontmatter field; see
[vault-layout-and-views.md](vault-layout-and-views.md)). There is no `view
edit` in v1; editing a view is remove + add.

- `view list` — list configured views as the aligned `NAME FIELD` table.
- `view add <name> --field <field>` — define a new view and materialize it (the
  view's directory is its name; grouping values are always normalized, ADR
  0009/0023, so there is no case flag). The name must not be reserved
  (`all-notes`, `.ntropy`, `.gitignore`) or already in use. Adding a view also
  records its directory in `.gitignore` (ADR 0032).
- `view remove <name>` — delete a view definition and prune its `.gitignore`
  entry. The view's directory is left on disk (ntropy never deletes a
  directory) and reported so you can remove it yourself.

### `tags`

List all distinct full tag strings across the vault with their note counts,
sorted alphabetically, as the aligned `TAG COUNT` table.

### `info`

Report the active vault and which rule resolved it (`--vault`, `$NTROPY_VAULT`,
a `.ntropy-vault` pointer, the current-directory walk-up, or the global
default), the configured global default vault, and vault statistics: note, tag,
view and template counts, the number of notes skipped with warnings, the
creation-date span, the most-used tags, and the template names. Unlike the data
commands this is a human report, printed the same way piped or on a TTY.

### `lsp`

Run the ntropy language server over stdin/stdout, for an editor's LSP client to
spawn. Unlike the other commands it does not resolve a vault from the global
flags; it resolves one per open document (ADR 0029). The capabilities and
editor setup are documented in
[language-server.md](language-server.md).
