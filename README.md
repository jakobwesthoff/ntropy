# ntropy

> An opinionated Markdown note-taking CLI where metadata, not folders, is the
> filing system.

ntropy manages a collection of notes that are plain Markdown files with YAML
frontmatter, authored in your own `$EDITOR`. Organization lives in the
frontmatter — tags, dates, and arbitrary fields — rather than in a folder
hierarchy you maintain by hand.

The tool is deliberately opinionated: notes live flat in a single vault, a
note's identity is stable and independent of its title, and any hierarchy you
want to browse is a derived projection of the metadata rather than the
canonical storage.

## How it works

A vault is a directory with a few well-known children:

    <vault>/
      all-notes/   canonical notes, named <ulid>-<slug>.md (the source of truth)
      by-tag/      a materialized view: a symlink tree grouped by the tags field
      .ntropy/     config and templates

Each note's identity is the leading ULID in its filename, so renaming the title
never breaks links. The readable date and any browsable hierarchy are *derived*:
views are symlink trees pointing back into `all-notes/`, regenerated on demand.

Unix only (macOS, Linux); views rely on symlinks.

## Install

    cargo install --path .   # from a checkout

Set `$VISUAL` or `$EDITOR`; ntropy opens notes in it and refuses to guess.

## Quick start

    ntropy init ~/notes          # scaffold a vault (seeds a by-tag view)
    cd ~/notes
    ntropy new My first note     # create from the template, then open the editor

Inside a vault, ntropy finds it by walking up from the current directory, so no
flags are needed. From elsewhere, point at it with `--vault <path>`,
`$NTROPY_VAULT`, a `.ntropy-vault` pointer file, or a global default vault
(`ntropy init --set-default`).

## Commands

- `init [path]` — scaffold (or complete) a vault; idempotent. The target is
  `path` or, if omitted, `--vault` (passing both is an error; with neither it
  uses the current directory). `--set-default` records it as the global default.
- `new <title>` — create a note from a template and open it. `--template`/`-t
  <name>` picks a template (see [Templates](#templates)); `--no-edit`
  (`--print`) just prints the path.
- `today` — open today's note, creating it from the `today` template on first
  use that day (see [Templates](#templates)). `--no-edit` (`--print`) just prints
  the path.
- `search [query]` — the one browse/filter/full-text entry point. On a TTY it
  opens an interactive fuzzy picker; piped or with `-n` it prints plain lines.
  The picker is bottom-anchored: the prompt sits at the bottom and the list
  grows upward with the best match nearest it. Type to filter; Ctrl-W deletes a
  word and Ctrl-U clears the query; Up/Ctrl-P move toward worse matches and
  Down/Ctrl-N toward the best; Enter opens the selection; Esc/Ctrl-C aborts.
  Matches are highlighted in yellow and the selected row is cyan, using your
  terminal's own colors so it adapts to your theme.
- `edit <id|query>` — open a note directly. A full ULID resolves to that note;
  anything else is a query. Ambiguous matches open a pre-filtered picker (or
  error when non-interactive).
- `delete <id|query>` — remove a note and refresh views (`-f` skips the prompt).
- `reconcile` — realign filenames whose slug drifted from the title, and rebuild
  every view (catches up after edits made outside ntropy). Prints each rename
  and a summary of notes scanned, files renamed, views rebuilt and warnings.
- `view list|add|remove` — manage views, e.g. `ntropy view add by-status --field status`.
- `tags` — list every tag with its note count.
- `info` — show the active vault and how it was resolved, the global default
  vault, and stats: note/tag/view/template counts, skipped-note warnings, the
  creation-date span, the top tags, and the template names.
- `lsp` — run the language server over stdin/stdout for your editor (see
  [Linking and the language server](#linking-and-the-language-server)).

Global flags (any command): `--vault <path>`, `-n`/`--non-interactive`,
`--strict` (treat malformed/badly-named notes as errors instead of warnings).

Non-interactive output is a tab-separated `id<TAB>date<TAB>title<TAB>tags<TAB>path`
table, newest first (tags comma-joined), led by an uppercase column header so it
is self-describing; `awk`/`cut` still work directly and `tail -n +2` drops the
header. The `tags` and `view list` tables carry a header too. Where a note is
named to you (delete prompts, ambiguous matches) it is shown as
`date  title  [tags]  (id)`.

## Query language

`search`, `edit` and `delete` share one DSL (precedence `not` > `and` > `or`,
parentheses override):

    ntropy search tag:work and not status:done
    ntropy search 'status:"in progress"'
    ntropy search 'text:"deadline" or tag:urgent'
    ntropy search rust            # a bare word is shorthand for text:rust

- `tag:x` — hierarchical match: `x`'s `/`-segments must appear as a contiguous
  run anywhere in a note tag (`tag:programming` matches `programming/rust` and
  `area/programming`). Case-insensitive.
- `field:value` — frontmatter equality, or membership for a list field. Quote
  multi-word values.
- `text:…` (and bare words/phrases) — a regex over the note body, smart-case
  (case-insensitive unless the pattern has an uppercase letter).

## Views

A view pairs an output directory with a frontmatter field. `by-tag` (on `tags`)
is seeded by `init`; add more like `ntropy view add by-status --field status`.
List fields fan a note out across values, a `/` in a value nests into
subdirectories, and grouping values are normalized (lowercased, slugified). The
result is a browsable symlink tree any tool (a file manager, `grep`, your
editor) can navigate, kept fresh after every ntropy mutation and by
`reconcile`.

## Templates

Templates are Markdown-with-frontmatter files in `<vault>/.ntropy/templates/`.
`init` seeds `default.md`. To add a note type, drop another file in that
directory; its name (without `.md`) is how you select it:

    ntropy new Standup --template meeting   # uses .ntropy/templates/meeting.md
    ntropy new Some note                     # uses default.md

`new` without `--template` uses `default.md` (falling back to a built-in default
if it is absent). A named template that does not exist is an error rather than a
silent fallback, so a typo is caught.

When a note is created, these placeholders are substituted in the template:

- `{{title}}` — the title you passed to `new`.
- `{{id}}` — the note's ULID.
- `{{date}}` — the creation date (`YYYY-MM-DD`, local timezone).
- `{{slug}}` — the slugified title.

Unknown placeholders are left untouched.

### The `today` template

`init` also seeds `today.md`, which powers the `today` command:

    ---
    title: {{date}}
    tags: [daily]
    ---
    # {{date}}

`ntropy today` opens today's note, identified by its title being today's date.
The first run on a given day creates it from this template; later runs the same
day reopen the same note rather than making a new one. Customize `today.md` to
change what your daily note looks like (it must exist; a vault created before
this feature can re-run `ntropy init` to seed it).

## Linking and the language server

Notes link to each other with ordinary Markdown links whose target is the note's
filename, `[display](<ulid>-<slug>.md)`. The leading 26-character ULID carries
identity, so a link keeps resolving even after the target's title (and slug)
change; `ntropy reconcile` refreshes the slug in existing links so they stay
clickable in any Markdown viewer. You can write these by hand, but the ergonomic
way is the language server.

`ntropy lsp` speaks the Language Server Protocol over stdin/stdout, so any
LSP-capable editor can use it. It provides:

- **Link completion** — type `[` and pick a note (fuzzy-matched on title and
  tags); the whole `[Title](<ulid>-<slug>.md)` is inserted. Typing inside a
  hand-written `](…)` completes just the target.
- **Tag completion** — inside a note's `tags:` frontmatter (both `[a, b]` and
  `- a` list forms), hierarchy-aware against the tags already in your vault.
- **Go to definition** and **document links** — jump to or click a link's target.
- **Workspace symbols** — jump to any note by title across the vault.

The server resolves the vault per open document (the same rules the CLI uses), so
no project configuration is required beyond pointing your editor at the binary.

### Neovim

For a recent Neovim (0.11+), start the server for Markdown buffers that live in a
vault. Put this in your config:

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "markdown",
  callback = function(args)
    local root = vim.fs.root(args.buf, { ".ntropy", ".ntropy-vault" })
    if not root then
      return -- not inside an ntropy vault
    end
    vim.lsp.start({
      name = "ntropy",
      cmd = { "ntropy", "lsp" },
      root_dir = root,
    })
  end,
})

-- Optional: snippet support makes `[` completion place the cursor after the link.
-- (Neovim's built-in client advertises it; nvim-cmp/blink users get it too.)
vim.keymap.set("n", "gd", vim.lsp.buf.definition)
vim.keymap.set("n", "<leader>fn", vim.lsp.buf.workspace_symbol) -- find note by title
```

`ntropy` must be on your `PATH` (e.g. via `cargo install ntropy`). Open a note
under `all-notes/`, type `[`, and the completion menu lists your notes; `gd`
follows a link, and the workspace-symbol picker jumps to any note by title.

## Development

Common tasks are wrapped as [`just`](https://github.com/casey/just) recipes
(`just --list`):

    just test      # cargo test
    just clippy    # cargo clippy --all-targets -- -D warnings
    just fmt       # cargo fmt
    just check     # clippy + tests + fmt --check (the CI gate)
    just coverage  # cargo llvm-cov

Tests use [`insta`](https://insta.rs) snapshots across all layers (ADR 0021).
When a change alters output, the snapshot assertions fail and write
`.pending-snap` files; review and accept them with
[`cargo-insta`](https://insta.rs/docs/cli/):

    cargo insta review   # interactively accept/reject pending snapshots
    cargo insta accept   # accept all pending snapshots

## Design

The full design is recorded as decision records under [`docs/adr/`](docs/adr/)
and narrative documents under [`docs/design/`](docs/design/).

## License

ntropy is licensed under the Mozilla Public License 2.0. See [`LICENSE`](LICENSE).
