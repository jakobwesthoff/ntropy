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
  In the picker, type to filter; Ctrl-W deletes a word and Ctrl-U clears the
  query; Up/Ctrl-P and Down/Ctrl-N move; Enter opens the selection; Esc/Ctrl-C
  aborts. The selection bar uses reverse video, so it adapts to your terminal
  theme.
- `edit <id|query>` — open a note directly. A full ULID resolves to that note;
  anything else is a query. Ambiguous matches open a pre-filtered picker (or
  error when non-interactive).
- `delete <id|query>` — remove a note and refresh views (`-f` skips the prompt).
- `reconcile` — realign filenames whose slug drifted from the title, and rebuild
  every view (catches up after edits made outside ntropy). Prints each rename
  and a summary of notes scanned, files renamed, views rebuilt and warnings.
- `view list|add|remove` — manage views, e.g. `ntropy view add by-status --field status`.
- `tags` — list every tag with its note count.

Global flags (any command): `--vault <path>`, `-n`/`--non-interactive`,
`--strict` (treat malformed/badly-named notes as errors instead of warnings).

Non-interactive output is a tab-separated `id<TAB>date<TAB>title<TAB>tags<TAB>path`
table, newest first (tags comma-joined), so `awk`/`cut` and pipelines work
directly. Where a note is named to you (delete prompts, ambiguous matches) it is
shown as `date  title  [tags]  (id)`.

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
