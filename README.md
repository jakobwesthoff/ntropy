# ntropy

An opinionated Markdown note-taking and management CLI where metadata, not
folders, is the filing system. No database, no proprietary app, no folder
hierarchy to maintain by hand — just plain Markdown files and their frontmatter.

The short version: write Markdown, tag it, and let ntropy do the filing — fuzzy
full-text search, a real query language, browsable views materialized straight
into your filesystem, and editor-native link and tag completion over LSP, with
zero folders to maintain by hand and zero databases pretending to be a note app.

## Why I built this

I live on the command line and in Neovim, and every note app I tried wanted me
back inside its own (usually graphical) UI to make sense of files it nominally
stored as plain text. I wanted the inverse: notes that are *just* Markdown
files, a CLI to manage them, and nothing stateful in between. ntropy is the
heavily opinionated result — notes live flat in one vault, a note's identity is
a stable ULID rather than its title, and any hierarchy you browse is a derived
projection of the frontmatter instead of the canonical storage. Switching vaults
is cheap (one for work, one for private, or a per-project vault pinned by a
`.ntropy-vault` file in a repo), so documenting a project is the same motion as
any other note. It scratched my itch; maybe it scratches yours.

## Installation

```bash
cargo install ntropy
```

### Pre-built Binaries

Pre-built binaries are available on the
[GitHub Releases](https://github.com/jakobwesthoff/ntropy/releases) page for
macOS (Apple Silicon & Intel) and Linux (x86_64 & aarch64, statically linked).

> [!NOTE]
> ntropy supports macOS and Linux. Windows isn't supported — see
> [Limitations](#limitations).

## Quick Start

```bash
# Scaffold a vault (this also seeds a by-tag view)
ntropy init ~/notes
cd ~/notes

# Create a note from a template and open it in your editor
ntropy new My first note

# Open today's daily note (created on first use each day)
ntropy today

# Find and open notes: full query language, fuzzy picker when several match
ntropy search tag:work and not status:done
```

New notes are stamped out from [templates](#templates); `today` has its own
[daily template](#daily-notes-with-today); and `search` speaks a small
[query language](#query-language), popping an [interactive picker](#the-interactive-picker)
when more than one note matches.

You never have to tell ntropy which vault you mean from inside one — it finds it
for you. See [Finding the vault](#finding-the-vault).

<!-- docs:start -->
## Documentation

The notes *are* the database. ntropy keeps no index, cache, or hidden state: the
Markdown files in your [vault](#the-vault) are the single source of truth, and
every command reads them fresh. Everything else it shows you — readable dates,
[tag](#note-format) counts, the browsable [view](#materialized-views) trees — is
derived on demand and can be deleted and rebuilt at will.

## Note format

A note is a plain Markdown file with a YAML frontmatter block. The schema is
permissive on purpose: any fields you write are kept, and every one of them
becomes filterable just by existing.

```markdown
---
title: Q3 Planning
tags: [work, planning, area/roadmap]
status: in progress
due: 2026-07-01
---
# Q3 Planning

Whatever you want below the frontmatter.
```

Two fields carry special meaning; the rest are yours:

- **`title`** (required) is the canonical, human title — full case, punctuation,
  and Unicode. The filename slug is derived from it, so the title is the truth
  and the slug is just a readable echo. A note with no `title` is treated as
  malformed (skipped with a warning, or an error under `--strict`).
- **`tags`** is a flat list of strings. A forward slash denotes hierarchy by
  convention: `area/roadmap` is one tag with two levels, which both
  [queries](#query-language) and [views](#materialized-views) understand.
- **Everything else** (`status`, `due`, `author`, anything you like) is a free
  field. Filter on it, build a view from it, or just keep it for yourself.

You never write the date or id by hand: a note's id *is* the ULID in its
filename, and its creation date is derived from it. And when ntropy rewrites a
note (during `reconcile`, say), any fields it doesn't recognize are preserved
untouched.

## The vault

A vault is an ordinary directory with a few well-known children:

```bash
~/notes/
├── all-notes/        # your notes, named <ulid>-<slug>.md — the source of truth
│   ├── 01j8z9k…-groceries.md
│   └── 01j8za2…-q3-planning.md
├── by-tag/           # a materialized view: symlinks grouped by the `tags` field
├── by-status/        # another view, grouped by the `status` field
└── .ntropy/          # config and templates (the only reserved directory)
```

Only top-level `*.md` files in `all-notes/` are notes. Subdirectories and
non-`.md` files are left alone, so you can keep images and attachments right next
to your notes without ntropy adopting them as notes.

Because all of this is just files, the whole vault is yours to version: `git
init` in it and commit your notes like any other text. The derived `by-*/` view
directories don't belong in git, and ntropy keeps them out for you: it maintains
a root `.gitignore` whose entries always match your configured views, adding one
when you add a view and pruning it when you remove one. Your own lines in that
file are never touched.

ntropy never deletes a directory. When a view is removed its directory is left
behind (and, no longer ignored, it shows up in `git status`); the command tells
you so you can delete the stale tree yourself.

### Finding the vault

Every command operates on exactly one vault, resolved in this order:

1. `--vault <path>`
2. `$NTROPY_VAULT`
3. A walk up from the current directory to the nearest ancestor holding a
   `.ntropy-vault` pointer file or a `.ntropy/` directory (nearest wins; a
   pointer beats a `.ntropy/` in the same directory, since it is an explicit
   redirect).
4. The global default vault (set with `ntropy init --set-default`).

Step 3 is the fun one. A `.ntropy-vault` file is a single line naming a vault
elsewhere — a path relative to the file, absolute, or `~`. Drop one at the root
of a project and ntropy uses that project's vault from anywhere inside it, so
project notes become the same `ntropy new` / `ntropy search` muscle memory as
everything else. A broken pointer is a hard error, never a silent fall-through to
the default.

## A day with ntropy

A quick tour of how the pieces fit together. Start with a thought:

```bash
ntropy new Refactor the parser
```

Your editor opens on a fresh note from `default.md`. Give it some frontmatter and
save:

```markdown
---
title: Refactor the parser
tags: [work, programming/rust]
status: in progress
---
```

Later, find it again — by tag, by status, by a word you half-remember:

```bash
ntropy search tag:work and status:"in progress"
```

A single match opens straight away; several drop you into the fuzzy picker.
Decide you browse by status often, so turn it into a view:

```bash
ntropy view add by-status --field status
```

Now `by-status/in-progress/` is a real folder of symlinks you can `cd` into,
`grep`, or open in any editor, no ntropy required. Edit a note's `status` outside
ntropy (straight in your editor, say) and the views won't know until you tell
them:

```bash
ntropy reconcile
```

That realigns any drifted filenames and re-syncs every view, and you're back in
sync.

## Commands

| Command | What it does |
|---------|--------------|
| `init [path]` | Scaffold (or complete) a vault; idempotent. Target is `path` or, if omitted, `--vault` (both is an error; neither uses the cwd). `--set-default` records it as the global default. |
| `new <title>` | Create a note from a [template](#templates) and open it. `--template`/`-t <name>` picks a template; `--no-edit` (`--print`) just prints the path. |
| `today` | Open today's note, creating it from the [`today` template](#daily-notes-with-today) on first use that day. `--no-edit` (`--print`) just prints the path. |
| `search [id\|query]` | The one browse/filter/full-text/open entry point (alias `list`). Speaks the [query language](#query-language) and opens the [picker](#the-interactive-picker) when several notes match. |
| `delete <id\|query>` | Remove a note and refresh views (`-f` skips the prompt). Must resolve to exactly one note, erroring on an ambiguous selector when non-interactive. |
| `reconcile` | Realign filenames whose slug drifted from the title and re-sync every view (catches up after edits made outside ntropy). |
| `view list\|add\|remove` | Manage [materialized views](#materialized-views), e.g. `ntropy view add by-status --field status`. |
| `tags` | List every tag with its note count. |
| `info` | Show the active vault and how it was resolved, the global default, and stats: note/tag/view/template counts, skipped-note warnings, the creation-date span, the top tags, and the template names. |
| `lsp` | Run the [language server](#language-server) over stdin/stdout for your editor. |

Global flags (any command): `--vault <path>`, `-n`/`--non-interactive`,
`--strict` (treat malformed or badly-named notes as errors instead of warnings).

## Query language

`search` (and `delete`) take a small query language. The fastest way to learn it
is to watch it work:

```bash
# "What was I supposed to do for work that isn't done yet?"
ntropy search tag:work and not status:done

# "That meeting note where somebody said 'deadline'..."
ntropy search text:deadline and tag:meeting

# "Everything still in progress, or anything that's on fire."
ntropy search 'status:"in progress" or tag:urgent'

# "I know I wrote 'borrow checker' somewhere in here."
ntropy search borrow checker

# "Just show me the whole pile." (no query at all)
ntropy search
```

Bare words are the lazy path: anything that isn't a `thing:value` term is matched
against the note body, so `ntropy search borrow checker` does exactly what you'd
hope. When you want precision, reach for the typed terms:

- **`tag:x`** matches hierarchically. `tag:programming` finds `programming`,
  `programming/rust`, *and* `area/programming`, because your query's
  `/`-segments just have to appear as a contiguous run somewhere in the tag.
  Case-insensitive.
- **`field:value`** is frontmatter equality (or membership, for list fields).
  Quote multi-word values: `status:"in progress"`.
- **`text:…`** is a regex over the note body, smart-case: an all-lowercase
  pattern matches anything, but slip in a capital and it turns case-sensitive.

Stitch terms together with `and`, `or`, and `not`, and reach for parentheses when
the precedence (`not` > `and` > `or`) isn't what you meant:

```bash
ntropy search '(tag:work or tag:side-project) and not status:done'
```

## The interactive picker

When `search` matches several notes on a terminal it opens a fuzzy picker; a
single match skips straight to opening the note, and a full ULID jumps right to
it. (Piped or with `-n` there's no picker at all — see [Scripting](#scripting).)

It's bottom-anchored, like a shell prompt: the input line sits at the bottom and
results stack upward, best match closest to your cursor. Type to filter live.
Matches glow yellow and the current row is cyan, drawn from your terminal's own
palette so it follows your theme.

| Key | Action |
|-----|--------|
| _type_ | Filter the list |
| `Down` / `Ctrl-N` | Move toward the best match |
| `Up` / `Ctrl-P` | Move toward worse matches |
| `Ctrl-W` | Delete the last word |
| `Ctrl-U` | Clear the query |
| `Enter` | Open the selected note |
| `Esc` / `Ctrl-C` | Abort |

Choose a note and ntropy opens it in your editor. When you close the editor it
quietly reconciles that note — the same realignment `ntropy reconcile` does
vault-wide — fixing its filename slug to match the current title and refreshing
any links that point at it.

## Materialized views

ntropy stores notes flat, with no folders to file them into. So how do you
*browse*? That's what views are for. A view is a question you ask once — "group
my notes by status", "by tag", "by project" — and then get to answer with plain
filesystem navigation forever after.

A view materializes that grouping as a real directory of symlinks pointing back
into `all-notes/`:

```bash
by-status/
├── done/
│   └── 01j8za2…-q3-planning.md -> ../../all-notes/01j8za2…-q3-planning.md
├── in-progress/
└── todo/
```

Because the leaves are symlinks to the canonical files, there is still exactly
one copy of every note; the view is just another door into it. `cd` into it,
`grep` it, point a file browser at it, open the links in any editor. It refreshes
automatically after every ntropy command that changes notes, and `ntropy
reconcile` brings it back in sync after out-of-band edits.

You control views per vault with `ntropy view`:

```bash
ntropy view add by-status --field status   # group notes by their `status` field
ntropy view list                            # show configured views
ntropy view remove by-status                # tear one down
```

`init` seeds a `by-tag` view (on the `tags` field) to get you started. Add one
for whatever frontmatter field you actually navigate by — `status`, `project`,
`author`, `area`, anything you put in your [notes](#note-format). List-valued
fields (like `tags`) fan a note out into every value it holds; a `/` inside a
value (`area/roadmap`) nests into subdirectories; and grouping values are
normalized (lowercased and slugified) so `In Progress` and `in-progress` land in
the same place.

Worth saying out loud: views are a convenience for when filesystem access is what
you want, not the only way to slice your notes. Every field a view can group by,
the [query language](#query-language) can filter by too — `ntropy search
status:done` needs no view at all. Make a view when you'll browse a dimension
often; reach for `search` for everything else.

## Templates

Every new note starts from a template, so every note can start with the
frontmatter and skeleton it *should* have instead of a blank file you furnish by
hand each time. Define the shape of a "meeting note" or a "book review" once, and
`ntropy new` stamps it out for you.

Templates are Markdown-with-frontmatter files in `<vault>/.ntropy/templates/`,
and the filename (minus `.md`) is the template's name:

```markdown
<!-- .ntropy/templates/meeting.md -->
---
title: {{title}}
date: {{date}}
tags: [meeting]
status: notes
---
# {{title}}

## Attendees

## Notes

## Action items
```

```bash
ntropy new Standup --template meeting   # uses meeting.md
ntropy new Some thought                  # uses default.md
```

`init` seeds a `default.md`, used whenever you don't pass `--template` (with a
built-in fallback if it is missing). Asking for a template that doesn't exist is
an error rather than a silent fall-back, so a typo never quietly hands you the
wrong shape.

The `{{...}}` placeholders are filled in at creation time:

| Placeholder | Becomes |
|-------------|---------|
| `{{title}}` | the title you passed to `new` |
| `{{id}}` | the note's ULID |
| `{{date}}` | the creation date — `YYYY-MM-DD`, local time |
| `{{slug}}` | the slugified title |

Anything ntropy doesn't recognize is left untouched, so a stray `{{mustache}}` in
your prose survives intact.

### Daily notes with today

`init` also seeds a `today.md` template, which powers `ntropy today`:

```markdown
---
title: {{date}}
tags: [daily]
---
# {{date}}
```

`ntropy today` opens today's note — identified by its title being today's date —
creating it from this template the first time you run it on a given day and
reopening the same note on later runs. Edit `today.md` to shape your daily note
however you like. (It has to exist; a vault created before this feature can
re-run `ntropy init` to seed it.)

## Configuration

There is not much to configure, on purpose. Three things are worth knowing.

**Your editor.** ntropy opens notes in `$VISUAL`, then `$EDITOR`. It deliberately
won't guess a default, so set one of those in your shell and ntropy uses it for
`new`, `today`, and opening notes from the picker.

**Your default vault.** `ntropy init --set-default` records a vault as the global
fallback, used when nothing nearer resolves (see
[Finding the vault](#finding-the-vault)). It lives in a small TOML file in your
OS config directory — `~/.config/ntropy/config.toml` on Linux,
`~/Library/Application Support/ntropy/config.toml` on macOS — holding a single
line:

```toml
default_vault = "/Users/you/notes"
```

You'll rarely touch it by hand; `--set-default` writes it for you.

**Your views.** Each vault's views are configured in `<vault>/.ntropy/config.toml`
so they travel with the vault rather than your machine. The `ntropy view`
commands manage this file for you — see [Materialized views](#materialized-views)
for the whole story.

## Linking between notes

Notes link to each other with ordinary Markdown links — nothing custom:

```markdown
See [the Q3 plan](01j8za2…-q3-planning.md) for the numbers.
```

The target is simply the note's filename. Because the leading ULID is the note's
real identity, the link keeps resolving even after the target's title and slug
change; `ntropy reconcile` rewrites the slug portion in existing links so the
readable part stays accurate. They're ordinary Markdown links, so GitHub, your
editor's preview, and any other Markdown tool follow them for free.

You can type these by hand, but you don't have to — that's what the
[language server](#language-server) is for.

## Language server

`ntropy lsp` runs a [Language Server](https://microsoft.github.io/language-server-protocol/)
over stdin/stdout. An LSP server is the same machinery that gives your editor
autocomplete and go-to-definition for code; here it teaches any LSP-capable
editor to understand an ntropy vault, turning the fiddly parts of note-taking
into ordinary editor features:

- **Link completion** — type `[` and pick a note (fuzzy-matched on title and
  tags); ntropy inserts the whole `[Title](<ulid>-<slug>.md)` for you, so
  [links](#linking-between-notes) never mean hand-copying a ULID. Typing inside
  an existing `](…)` completes just the target.
- **Tag completion** — inside a note's `tags:` frontmatter, completion is
  hierarchy-aware against the [tags](#note-format) already in your vault, in both
  `[a, b]` and `- a` list forms.
- **Go to definition & document links** — jump to or click straight through a
  link to the note it points at.
- **Workspace symbols** — jump to any note in the vault by title.

It resolves the vault per open document using the same rules as the CLI, so there
is nothing to configure beyond pointing your editor at the binary.

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

`ntropy` must be on your `PATH`. Open a note under `all-notes/`, type `[`, and the
completion menu lists your notes; `gd` follows a link, and the workspace-symbol
picker jumps to any note by title.

## Scripting

Pipe ntropy anywhere, or pass `-n`/`--non-interactive`, and it drops all the
interactive niceties: no picker, no editor, just plain text on stdout. `search`
then prints one note per line as a tab-separated table:

```
ID<TAB>DATE<TAB>TITLE<TAB>TAGS<TAB>PATH
```

newest first, tags comma-joined, led by an uppercase header row so the output is
self-describing. `awk` and `cut` work on it directly, and `tail -n +2` drops the
header. (`tags` and `view list` print headers too.)

Exit codes are scriptable: a `search` that matches nothing exits non-zero, so
`if ntropy search -n tag:urgent; then …` branches on "did anything match" without
parsing a single line. Where a note has to be named back to you (a delete prompt,
an ambiguous match) it's shown as `date  title  [tags]  (id)`.

## Limitations

- **macOS and Linux only.** Views are real symlink trees, which Windows makes
  awkward — the trade for views you can `cd` into. Not supported on Windows yet.
- **Happiest at personal scale.** Your files *are* the database and ntropy
  re-reads them each run instead of consulting an index, so it's tuned for the
  low thousands of notes, not hundred-thousand-note archives. In return,
  everything stays plain, greppable, committable files — and there's room to add
  caching later without resorting to a real database.
- **Views can drift on out-of-band edits.** Change frontmatter or rename files
  behind ntropy's back and the views won't catch up until the next `ntropy
  reconcile` — one command away.
<!-- docs:end -->

## Development

```bash
git clone https://github.com/jakobwesthoff/ntropy.git
cd ntropy
cargo build --release
cargo install --path .   # install your working copy onto your PATH
```

Common tasks are wrapped as [`just`](https://github.com/casey/just) recipes
(`just --list`):

```bash
just test      # cargo test
just clippy    # cargo clippy --all-targets -- -D warnings
just fmt       # cargo fmt
just check     # clippy + tests + fmt --check (the CI gate)
just coverage  # cargo llvm-cov
```

Tests use [`insta`](https://insta.rs) snapshots across all layers (ADR 0021).
When a change alters output, the snapshot assertions fail and write `.pending-snap`
files; review and accept them with [`cargo-insta`](https://insta.rs/docs/cli/):

```bash
cargo insta review   # interactively accept/reject pending snapshots
cargo insta accept   # accept all pending snapshots
```

## Design

The full design is recorded as decision records under [`docs/adr/`](docs/adr/)
and narrative documents under [`docs/design/`](docs/design/).

## License

ntropy is licensed under the Mozilla Public License 2.0. See [`LICENSE`](LICENSE).

Copyright (c) 2026 Jakob Westhoff <jakob@westhoffswelt.de>
