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
├── README.md         # seeded by `init`: what this directory is, how to get ntropy
└── .ntropy/          # config and templates (the only reserved directory)
```

Only top-level `*.md` files in `all-notes/` are notes. Subdirectories and
non-`.md` files are left alone, so you can keep images and attachments right next
to your notes without ntropy adopting them as notes.

A vault can also store its notes [encrypted at rest](#encrypted-vaults), in
which case `all-notes/` holds `<ulid>.age` files instead and there are no view
directories.

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
| `new <title>` | Create a note from a [template](#templates) and open it. `--template`/`-t <name>` picks a template; `--empty` writes no content at all, for a caller that authors the note itself; `--print`/`-p` just prints the path. |
| `today` | Open today's note, creating it from the [`today` template](#daily-notes-with-today) on first use that day. `--print`/`-p` just prints the path. |
| `write <id\|filename\|path>` | Replace one note's content with text read from stdin, then realign and refresh views. Names its target rather than searching for it, never prompts, and works on an encrypted vault including a locked one. |
| `search [id\|query]` | The one browse/filter/full-text/open entry point (alias `list`). Speaks the [query language](#query-language) and opens the [picker](#the-interactive-picker) when several notes match. `--print`/`-p` prints the selected note's path instead of opening it; `--print-content`/`-P` prints the note's text (exactly one note). |
| `delete <id\|query>` | Remove a note and refresh views (`-f` skips the prompt). Must resolve to exactly one note, erroring on an ambiguous selector when non-interactive. |
| `render [id\|query]` | [Render one note to a PDF](#rendering-notes-to-pdf) with ntropy's own typst engine (only `typst` required on `PATH`), or with `--to html` to a web page with a `<stem>_files/` directory beside it, needing no tool. Must resolve to exactly one note; with no selector the picker opens over all notes, like `search`. `--to` picks the format (default `pdf`, or `typst` for the emitted Typst document), `-o` the output path (default `./<slug>.<ext>`), `--theme` overrides the vault's [configured theme](#theming-rendered-documents), `-p` prints the artifact path. An existing artifact is refused unless `--force` replaces it. |
| `site -o <dir> [query]` | [Export the vault as a static website](#exporting-the-vault-as-a-website): a page per note, a tag tree, a page per view and group, sidebar, outline, breadcrumbs. Works from disk without a server. `--force` empties a non-empty directory, `--theme` overrides the [site theme](#site-themes), `-p` prints the front page's path. `site theme init <name>` copies the built-in theme into the vault. |
| `reconcile` | Realign filenames whose slug drifted from the title and re-sync every view (catches up after edits made outside ntropy). |
| `view list\|add\|remove` | Manage [materialized views](#materialized-views), e.g. `ntropy view add by-status --field status`. |
| `tags` | List every tag with its note count. |
| `info` | Show the active vault and how it was resolved, the global default, and stats: note/tag/view/template counts, skipped-note warnings, the creation-date span, the top tags, and the template names. `--print`/`-p` prints the vault's path alone, for [shell integration](#shell-integration). |
| `unlock` / `lock` | Store or forget an [encrypted vault](#encrypted-vaults)'s key, so ordinary commands need no passphrase. |
| `vault encrypt\|decrypt\|rekey\|passphrase` | Convert a vault's storage, re-encrypt it to a fresh key, or change the passphrase. Each rewrites the whole vault, so each asks first (`-y` skips) and each is safe to interrupt (`--resume` finishes). |
| `lsp` | Run the [language server](#language-server) over stdin/stdout for your editor. |

Global flags (any command): `--vault <path>`, `-n`/`--non-interactive`,
`--strict` (treat malformed or badly-named notes as errors instead of warnings),
`-i`/`--identity <path>` and `--passphrase-file <path>` (see
[encrypted vaults](#encrypted-vaults)).

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

When `search` matches several notes it opens a fuzzy picker; a single match
skips straight to opening the note, and a full ULID jumps right to it. The
picker draws on your terminal even while stdout feeds a pipe, so
`ntropy search -p | pbcopy` picks interactively and pipes just the chosen
note's path. (With `-n` there's no picker at all — see
[Scripting](#scripting).)

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

> [!NOTE]
> Views are unavailable in an [encrypted vault](#encrypted-vaults): a symlink
> tree would spell out your tag taxonomy in plaintext directory names inside
> the synced folder.

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

## Encrypted vaults

A vault can store its notes encrypted at rest, so the thing that syncs your
notes cannot read them:

```bash
ntropy init ~/private --encrypted     # asks for a passphrase
```

Everything else works as it always did. After a one-time `ntropy unlock` the
key lives in your OS credential store and `new`, `search`, `render` and the rest
behave exactly as in a plaintext vault. `ntropy lock` forgets it again.

**What this protects, and what it doesn't.** Encryption defends the *content* of
your notes against whoever stores or syncs the vault directory — Dropbox,
iCloud, a git host, anyone who ends up with a copy. They see ciphertext. It does
not hide metadata: filenames still reveal each note's id and therefore its
creation time, along with how many notes you have and how big each one is, and
`.ntropy/` stays readable. Nor does it reach backwards: if the vault was
plaintext and synced before you encrypted it, those old revisions are still
readable in your provider's version history, and cleaning that up is your job.

```bash
ntropy vault encrypt      # convert an existing vault
ntropy vault decrypt      # and back again
ntropy vault rekey        # re-encrypt everything to a fresh key (same passphrase)
ntropy vault passphrase   # change the passphrase; notes are untouched
```

Each of those rewrites the whole vault, so each asks first (`-y` skips) and each
is safe to interrupt: every note is written in its new form and verified against
the old one *before* anything is deleted, so a conversion killed halfway leaves
both copies on disk. The next command refuses to touch the vault and tells you
to finish with `--resume`.

**Writing needs no key.** The vault has one keypair, and only the public half is
needed to encrypt. `ntropy new` therefore works on a locked vault — you can jot
something down without unlocking anything. Reading, searching and editing need
the key, so `ntropy today` (which finds today's note by title) does not.

**For scripts and headless machines**, `--identity <path>` or `$NTROPY_IDENTITY`
names an age identity file to use instead of the credential store, and
`--passphrase-file <path>` supplies a passphrase from a file's first line rather
than a prompt. Neither writes anything to your credential store — only a
passphrase you actually typed leaves the vault unlocked afterwards. With `-n`,
ntropy never prompts at all: it fails with a message naming `ntropy unlock`
rather than blocking on a terminal that isn't there.

> [!NOTE]
> `--print`/`-p` reports the real path, which in an encrypted vault is the
> ciphertext file — fine for `stat` or `xargs rm`, not for reading. Use
> `search -P`/`--print-content` to get the note's text instead, which reads the
> same in either kind of vault. Editing by hand goes through `ntropy search`;
> `ntropy write` puts content back without an editor, which is what makes an
> encrypted vault scriptable.

Two things behave differently in an encrypted vault. [Materialized
views](#materialized-views) are disabled, because a `by-tag/` symlink tree would
spell out your whole tag taxonomy in plaintext directory names inside the synced
folder. And a rendered PDF is plaintext by nature, so if you write one *into*
the vault ntropy warns you that it will sync unencrypted.

It's all standard [age](https://age-encryption.org) underneath — every file
ntropy writes is a plain age file. With the stock `age` CLI and your passphrase
you can recover a vault without ntropy at all.

> [!NOTE]
> Encryption is a default-on cargo feature. `cargo install ntropy
> --no-default-features` builds without the cryptography dependencies; such a
> build recognizes an encrypted vault and says it cannot open it.

Full details: [docs/design/encryption.md](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/encryption.md).

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

To skip templating entirely, `ntropy new --empty <title>` creates the file with
no content in it. ntropy still picks the ULID, the location and the filename;
what goes inside is yours to write. That is mainly for scripts and agents, which
would otherwise have to parse and rewrite around a stamped skeleton. Until
frontmatter lands in the file it is not a well-formed note, so ntropy skips it
with a warning meanwhile.

`ntropy write` is the other half of that: it takes a note's whole text on stdin
and stores it, then realigns the filename and refreshes the views for you. The
two together author a note without ever reading one back, and without an editor,
in a plaintext and an encrypted vault alike:

```bash
path=$(ntropy new --empty -p Quarterly review)
ntropy write "$path" <<'EOF'
---
title: Quarterly review
tags: [work, planning]
---
# Quarterly review

Numbers go here.
EOF
```

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

There is not much to configure, on purpose. Four things are worth knowing.

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

**Your documents and your site.** The same file holds an optional `[render]`
table (the Typst theme and paper size, see
[Theming rendered documents](#theming-rendered-documents)) and an optional
`[site]` table (the site theme, the front page, the title, the language,
and the sidebar's root and nav table, see
[Exporting the vault as a website](#exporting-the-vault-as-a-website)).

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

## Rendering notes to PDF

`ntropy render` turns a single note into a typeset PDF — title, date, and tags
up top, the body below, and links to other notes shown by their current
titles:

```bash
# Render a note into ./<slug>.pdf in the current directory
ntropy render 01j8za2…

# Name the output yourself, and open the result in one go
open "$(ntropy render -p 01j8za2… -o q3-report.pdf)"
```

ntropy converts the note to Typst with its own engine and only typesets the
PDF with [typst](https://typst.app), so that is the single tool you install
yourself — for example via `brew install typst`. ntropy tells you exactly
what's missing if it isn't on your `PATH`.

`--to typst` writes the emitted Typst document (a `.typ` file) instead of a
PDF, needing no external tool at all. `--to html` writes the note as a web
page, the same page the [website export](#exporting-the-vault-as-a-website)
gives it, with a `<stem>_files/` directory beside it; that needs no tool
either and is described there. Whatever the format, `render` refuses to
overwrite an existing artifact (or, for html, a non-empty files directory)
unless you pass `--force`.

**Links between rendered notes.** A link to another note becomes a real link in
the PDF, pointing at `<slug>.pdf` — the target note's slug, which is also the
name `render` gives that note's artifact by default. Render a set of notes into
one directory and their cross-references line up:

```bash
# Rendered with no -o, each artifact is named after the note's own slug,
# which is exactly what the other one's link points at
cd out
ntropy render 01j8za2…   # writes ./architecture.pdf
ntropy render 01j8zb7…   # writes ./deployment.pdf
```

The link is a plain relative reference, so whether a click opens the other file
is up to your PDF viewer. It finds nothing if the target was never rendered, or
was rendered into a different directory or under a different name. Two notes
whose slugs are identical also share an artifact name, so a link to either
reaches whichever was written last.

### Theming rendered documents

A vault can render in its own livery. Drop a Typst file into
`.ntropy/themes/typst/` and name it in the `[render]` section:

```toml
# .ntropy/config.toml
[render]
theme = "corporate"
```

That is the whole setup. From then on every `ntropy render` in the vault uses
it, with nothing extra on the command line:

```bash
ntropy render 01j8za2…              # themed

# and so is every note in the vault
ntropy search -n | tail -n +2 | awk '{print $1}' |
  while read -r id; do ntropy render -n "$id"; done
```

A theme redefines what it wants and inherits the rest. This one puts a logo in
the page header and drops the metadata strip, so internal tags and
`status: draft` never reach a customer:

```typst
// .ntropy/themes/typst/corporate.typ
#let note(title: none, frontmatter: (:), paper: "a4", body) = {
  set document(title: title) if title != none

  set page(
    paper: paper,
    margin: (x: 2.2cm, top: 3.4cm, bottom: 2.4cm),
    header: {
      align(right, image("/assets/logo.svg", width: 3.2cm))
      v(-0.4em)
      line(length: 100%, stroke: 0.6pt + rgb("#2dd4bf"))
    },
  )
  set text(size: 11pt)

  if title != none {
    text(size: 1.6em, weight: "bold", title)
    v(0.8em)
  }

  // No frontmatter strip: nothing but the body below the title.
  body
}
```

The logo lives at `<vault>/assets/logo.svg` — outside `all-notes/`, which holds
notes and nothing else. Paths starting with `/` are relative to the vault root,
which is what the compiler is given access to.

Four functions are yours to override; a theme defining only `note` keeps the
built-in look for the others:

| Function | Signature | Renders |
| :--- | :--- | :--- |
| `note` | `note(title: none, frontmatter: (:), paper: "a4", body)` | the whole document |
| `callout` | `callout(kind: "note", body)` | a `> [!NOTE]` admonition |
| `notelink` | `notelink(body)` | a link to another note |
| `task` | `task(done: false)` | a task-list checkbox |

`--theme <name>` overrides the configured theme for one render, and
`--theme default` goes back to ntropy's built-in look. A theme that is missing
or does not compile fails the render: a document is never quietly produced in
the wrong livery.

> [!NOTE]
> Because a theme's assets are addressed from the vault root, compiling a
> `--to typst` artifact by hand takes `typst compile --root <vault> note.typ`.

**Paper size.** Rendering defaults to a4. A `[render]` section in the vault's
`.ntropy/config.toml` picks a different format:

```toml
[render]
paper = "us-letter"
```

Supported values: `a3`, `a4`, `a5`, `iso-b5`, `jis-b5`, `us-letter`,
`us-legal`, `us-tabloid`, `us-executive`, `us-oficio`. An unknown value is a
config error naming the bad name, reported before anything renders.

> [!NOTE]
> ntropy's rendering infrastructure is built around interchangeable rendering
> engines. Themes are per-vault Typst files; ntropy ships no named themes of
> its own beyond the built-in default.

> [!NOTE]
> A rendered artifact is plaintext by nature. Writing one into an
> [encrypted vault](#encrypted-vaults) means it syncs unencrypted, and ntropy
> warns when the output path lands there — most often when your shell happens
> to be sitting in the vault and the default `./<slug>.pdf` applies.

## Exporting the vault as a website

`ntropy site` turns the vault into a static website: a set of files any
web host serves, and that a browser opens straight from disk, no server
needed.

```bash
ntropy site -o ./public              # every note
ntropy site -o ./public tag:public   # only the notes a query selects
open "$(ntropy site -o ./public -p)" # export, then open the front page
```

The site mirrors the ways you reach a note in the vault. Every note is a
page under `notes/`. The tag hierarchy is a tree of pages under `tags/`,
each listing the notes carrying the tag or any tag below it. Every
[materialized view](#materialized-views) becomes a tree under `views/`,
nested like its directory (a view over `tags` is skipped, the tag pages
already are that view). Each page carries a sidebar with the views and
the top-level tags, breadcrumbs, an outline of the note's headings,
previous/next links that follow the sidebar's reading order, and, at
the end of a note, the notes that share the most tags with it. Tags link to their
pages everywhere they appear. Code blocks are highlighted in the browser
for 72 languages, each page loading only the grammars its code needs (a
fence language without one is an export warning, and the block stays
plain). Every page has a light/dark/system switch that remembers your
choice, and the outline follows your position while you scroll. Note
links point at the target's page; images and linked files from the vault
are copied under `files/`, a linked directory with its whole tree. The front page is the note named
by `[site] index` in the vault config, or a generated overview of the
newest notes, the top-level tags, and the views. On a phone the sidebar
is a drawer behind the menu button.

A note shapes its place in the navigation through a `site` table in its
frontmatter, every key optional:

```yaml
---
title: Basics
tags: [docs/start]
site:
  index: true            # this note is the landing page of docs/start
  listing: false         # true lists the group's contents below it
  label: Getting Started # what the sidebar calls it (and its group)
  order: 1               # its position; on a landing note, the group's
  hidden: false          # true keeps a note out of the sidebar and lists
  related: false         # no related notes under this page
---
```

Inside a group, the entries with an `order` come first, then the notes
without one newest first, then the child groups by name. A landing
note's title and body become its group's page, with the listing of
what the group holds below only when the note asks, and links to the
note go there. A hidden note keeps
its page and stays searchable; it just appears nowhere in the
navigation. Previous and next follow the sidebar's reading order across
groups, so a tag subtree with landing notes reads like a book.

Where the sidebar starts, and what it holds, is the vault config's
business. `root` starts it at one tag or view group instead of the
whole vault, which is what a documentation site wants; exporting with a
single `tag:` query does the same without config. A `[[site.nav]]`
table assembles the sidebar by hand instead, and is then all of it:

```toml
[site]
root = "tags/docs"                   # the sidebar is the docs subtree
related = false                      # no related notes under the pages

[[site.nav]]                         # or: sections listed by hand
label = "Getting Started"
items = [
  { note = "01ARZ3NDEKTSV4RRFFQ69G5FAV" },          # a note, by ULID
  { label = "Gadgets", tag = "docs/start/gadgets" }, # a tag's subtree
]

[[site.nav]]
label = "Reference"
items = [
  { view = "by-status", group = "open" },  # one group of a view
  { view = "by-status" },                  # a whole view
  { tags = true },                         # the whole tag tree
  { label = "More", items = [ ] },         # a group made by hand
]
```

An item that names something the export does not have is a warning
and is left out, so the site is still written. `related = false` drops
the related notes from every page, which a documentation tree wants,
since its pages all share the same tags; a note's own `site.related`
wins over it either way.

The search is a palette over the page: the header's button, `/`, or
Ctrl+K opens it, typing searches as you go, the arrow keys and Enter open
a result. It is made for readers rather than for the query language: a
word matches titles, tags, frontmatter values, and text, two words need
both, `tag:wis` finds `wisdome`, and a matching tag or view page appears
above the notes. The typed terms of the [query language](#query-language)
still narrow, `tag:work and not status:done` works, and `text:` is the
same regex as in the CLI. The search runs in the browser over data
exported with the site, so it works from disk.

A non-empty output directory is refused unless you pass `--force`, which
empties it first. A single note renders to the same page on its own with
`ntropy render --to html`: `report.html` plus a `report_files/` directory
beside it holding the theme, the fonts, the scripts, the grammars its
code needs, and the images it shows, the way a browser saves a page.
The page keeps the header, the scheme switch, and the outline, and drops
what needs a site: the sidebar, the search, breadcrumbs, and the pager.
`render` refuses to overwrite an existing artifact or a non-empty files
directory unless you pass `--force`.

### Site themes

A site theme is a directory `.ntropy/themes/site/<name>/`. The HTML
structure is ntropy's, so a theme changes the look, not the layout; what
it holds is what a web developer expects:

```
style.css        the entry point, required
icons/*.svg      one icon per file
fonts/*          files the stylesheet references with url(fonts/...)
anything else    copied into the site's assets/ as it is
```

Start from the built-in theme, which has the same layout:

```bash
ntropy site theme init mine          # writes .ntropy/themes/site/mine/
```

```toml
# .ntropy/config.toml
[site]
theme = "mine"
title = "Team Docs"                  # defaults to the vault directory name
index = "01ARZ3NDEKTSV4RRFFQ69G5FAV" # the note that becomes the front page
lang = "en"
```

`--theme <name>` overrides the configured theme for one export, and
`--theme default` returns to the built-in look.

**Colors and type.** Every color and typeface of the built-in theme is a
custom property on `:root`. The dark palette redefines them under
`prefers-color-scheme: dark` and under `:root[data-theme="dark"]`;
`data-theme="light"` wins over the system preference. A theme that only
wants different colors redefines these and keeps the rest:

| Property | Role |
|----------|------|
| `--bg`, `--surface`, `--raised` | the page ground, a tinted surface (code, callouts, hover fills), a raised panel |
| `--border` | rules and outlines |
| `--fg-bright`, `--fg`, `--muted`, `--faint` | headings, body text, secondary text, marks |
| `--accent`, `--accent-soft` | the accent and its translucent fill |
| `--link`, `--note-link` | ordinary links, links to other notes |
| `--callout-note`, `--callout-tip`, `--callout-important`, `--callout-warning`, `--callout-caution` | the five callout accents |
| `--display`, `--serif`, `--sans`, `--mono` | the font stacks for page titles, note bodies, the chrome and lists, dates and code |

**Sizes.** Named for their job on these pages, not as a generic scale, so
widening the reading column or flattening the corners is one property:

| Property | Role |
|----------|------|
| `--measure` | the reading column's width |
| `--page-width`, `--gutter` | the layout's outer width and side padding |
| `--sidebar-width`, `--outline-width`, `--column-gap`, `--header-height` | the chrome's dimensions |
| `--block-gap`, `--section-gap` | between the blocks of a note, between sections |
| `--radius-control`, `--radius-block`, `--radius-panel`, `--radius-pill` | buttons and inputs; code, quotes, tables, callouts; the search panel; chips |
| `--accent-bar` | the bar on code blocks, quotes, and callouts |
| `--shadow-panel`, `--shadow-raised` | the search panel, raised elements |
| `--motion` | the length of every transition |

**Fonts.** The built-in theme ships four faces under `fonts/`, all SIL Open
Font License (see `fonts/LICENSE`), declared with `@font-face` in
`style.css`:

| Face | Stack | Used for |
|------|-------|----------|
| Fraunces | `--display` | page titles |
| Literata | `--serif` | note bodies and their headings |
| DM Sans | `--sans` | the chrome and lists |
| DM Mono | `--mono` | dates, counts, and code |

Put your own files under `fonts/` and declare them the same way; the
stylesheet's `url()`s resolve relative to itself in the site's `assets/`.

**Icons.** Every `icons/<name>.svg` becomes a `<symbol id="icon-<name>">`
of a sprite inlined into every page, and the markup shows an icon with
`<svg class="icon"><use href="#icon-<name>"/></svg>`. Your theme's icons
are layered by name over the built-in set (Lucide, ISC, see
`icons/LICENSE`): a file with a built-in name replaces that icon, any
other name adds one, and a theme without `icons/` keeps them all. An icon
takes the text color, so the stylesheet sizes and colors it through the
`.icon` class. The names the pages use:

- the chrome: `menu`, `x`, `search`, `monitor`, `sun`, `moon`, `tag`,
  `chevron-right`, `chevron-left`;
- callouts: `info`, `lightbulb`, `message-square-warning`,
  `triangle-alert`, `octagon-alert`;
- the search palette's result kinds: `file-text`, `folder`, `tag`.

**Markup.** The skeleton of a page, with the classes a stylesheet targets
(`body.document` and no sidebar column for a `render --to html` page):

```html
<body class="site">
  <div class="layout">
    <header class="site-header">
      <a class="site-name">…</a>
      <button class="search-toggle">…</button>
      <div class="theme-switch">            <!-- three buttons -->
        <button class="theme-choice">…</button>
      </div>
    </header>
    <div class="sidebar-pane">
      <nav class="sidebar">
        <details class="nav-section">        <!-- one per view, or per nav table section -->
          <summary class="nav-title">…</summary>
          <a class="nav-all">…</a>           <!-- the section's own page, when it has one -->
          <ul class="nav-entries">
            <li class="nav-note"><a>…</a></li>
            <li class="nav-group">
              <details>
                <summary><svg class="icon chevron"/><a>…</a></summary>  <!-- a span for a hand-made group -->
                <ul class="nav-entries">…</ul>  <!-- nesting repeats -->
              </details>
            </li>
          </ul>
        </details>
        <section class="nav-section nav-tags"> <!-- the tag section -->
          <h2 class="nav-title"><a>…</a></h2>
          <ul class="tag-cloud"><li><a>… <span class="count">…</span></a></li></ul>
        </section>
      </nav>
    </div>
    <main>
      <nav class="breadcrumbs"><ol><li><a>…</a></li></ol></nav>  <!-- a span for a step without a page -->
      <div class="content">
        <header class="note-header">
          <h1 class="note-title">…</h1>
          <p class="note-meta">
            <time class="note-created">…</time>
            <span class="tags"><a class="tag"><svg class="icon"/>…</a></span>
          </p>
          <dl class="frontmatter">…</dl>     <!-- the remaining fields -->
        </header>
        <article class="note-body">…</article>
        <section class="related">
          <ol class="note-rows">              <!-- every list of notes -->
            <li class="note-row">
              <time>…</time>
              <span class="note-row-main">
                <a class="note-row-title">…</a>
                <span class="note-row-tags"><a class="tag">…</a></span>
              </span>
            </li>
          </ol>
        </section>
      </div>
      <nav class="pager"><a class="prev">…</a><a class="next">…</a></nav>
    </main>
    <aside class="side">
      <nav class="outline"><ul><li class="depth-2"><a>…</a></li></ul></nav>
    </aside>
  </div>
</body>
```

Tag and group pages list their child groups as `ul.group-chips` of `.chip`
links, each with a `.count`. In the body, callouts are
`.callout.callout-<kind>` with a `.callout-title`, and highlighted code
carries Shiki's `--shiki-light` and `--shiki-dark` colors per token, one
of which the stylesheet picks by scheme. The current sidebar entry and
outline entry carry `aria-current`. Read the built-in `style.css` for the
rest; it is written to be copied.

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

Pass `-n`/`--non-interactive` and ntropy drops all the interactive niceties:
no picker, no editor, just plain text on stdout. Piping alone doesn't do that
— ntropy stays interactive as long as it can reach your terminal, so scripts
and command substitutions must say `-n` (the same happens automatically where
no terminal exists, like cron or CI). `search -n`
prints one note per line as a space-aligned `ID DATE TITLE TAGS PATH`
table: each column is padded to its widest cell in Unicode display width and
separated from the next by two or more spaces, except the last column, which
is left unpadded so no line carries trailing whitespace. Rows are newest
first, tags comma-joined, led by an uppercase header row so the output is
self-describing.

Split it with `awk -F'  +'` (field separator: a run of two or more spaces),
for example `awk -F'  +' '{print $1}'` for the ID column; `tail -n +2` still
drops the header. One caveat: a cell can itself contain a two-space run (a
title with doubled spaces is rendered verbatim), which shifts the fields after
it — so treat TITLE and the columns after it as best-effort rather than a
positional contract. ID and DATE can never contain spaces and are always safe.
(`tags` and `view list` print headers too.)

File paths need no parsing at all: `search -n -p` prints every match as one
path per line (`ntropy search -n -p tag:work | xargs grep -l deadline`), and
`new -p`/`today -p` print the created note's path.

To read a note rather than locate it, `search -P`/`--print-content` writes its
text to stdout. It must resolve to exactly one note, and unlike `-p` it works
the same in an [encrypted vault](#encrypted-vaults) — the path there names a
ciphertext file, the content does not:

```bash
ntropy search -n -P 01J8Z9K…            # the note, either kind of vault
for id in $(ntropy search -n tag:work | tail -n +2 | awk '{print $1}'); do
    ntropy search -n -P "$id"
done
```

Against an [encrypted vault](#encrypted-vaults), `--identity <path>` or
`$NTROPY_IDENTITY` supplies the key without the OS credential store, and
`--passphrase-file <path>` supplies a passphrase without a prompt — which is
what makes an encrypted vault usable from cron or CI. Note that `-p` there
prints the ciphertext file's path: fine for `stat` or `xargs rm`, not for
handing to an editor.

Exit codes are scriptable: a `search` that matches nothing exits non-zero, so
`if ntropy search -n tag:urgent; then …` branches on "did anything match" without
parsing a single line. Where a note has to be named back to you (a delete prompt,
an ambiguous match) it's shown as `date  title  [tags]  (id)`.

## Shell integration

`ntropy info --print` prints the active vault's path and nothing else, resolved
by the [usual rules](#finding-the-vault) and always absolute. That's the hook
for shell functions, and the repo ships one:
[`contrib/shell/ntropy.sh`](https://github.com/jakobwesthoff/ntropy/blob/main/contrib/shell/ntropy.sh)
defines `ncd`, which drops you into the active vault.

```bash
# In ~/.bashrc or ~/.zshrc
source /path/to/ntropy/contrib/shell/ntropy.sh
```

```bash
ncd              # cd to whichever vault is active here
ncd --vault ~/notes   # or name one; every global flag is forwarded
```

It has to be a shell function rather than an `ntropy cd` subcommand, because a
process can't change its parent shell's working directory. Nothing is installed
for you; source the file yourself.

## Agent skill

LLM coding agents can drive ntropy well, since the CLI is fully scriptable,
but they have to know the house rules: always pass `-n`, create notes with `ntropy
new --print` instead of hand-writing files into `all-notes/`, run `ntropy
reconcile` after editing a note directly. The repo ships an agent skill that
teaches exactly that:
[`skills/ntropy/`](https://github.com/jakobwesthoff/ntropy/tree/main/skills/ntropy)
holds a `SKILL.md`
with the vault model and those rules, plus reference docs on writing notes,
querying, vaults, views, the website export with its navigation settings,
and site themes. Its description marks it as relevant to any task
involving ntropy or a note vault, so an agent with the skill installed picks it
up on its own the moment a task touches one.

The quickest install is the [`skills` CLI](https://skills.sh), which places the
skill into Claude Code, Cursor, Codex, and most other SKILL.md-aware agents.
It installs project-level by default and user-wide with `-g`:

```bash
npx skills add jakobwesthoff/ntropy
```

Or skip the tooling and copy the directory by hand. For Claude Code it belongs
at `~/.claude/skills/ntropy` (every project) or `<project>/.claude/skills/ntropy`
(just that project):

```bash
git clone https://github.com/jakobwesthoff/ntropy.git
cp -R ntropy/skills/ntropy ~/.claude/skills/ntropy
```

## Markdown flavor

Notes are GitHub-flavored Markdown: what GitHub renders is what ntropy
understands, so a vault reads the same on github.com, in your editor's
preview, and in a rendered PDF. The full supported surface:

| Feature | Syntax |
| :--- | :--- |
| Headings | `#` through `######` |
| Emphasis, strong, strikethrough | `*em*`, `**strong**`, `~~gone~~` |
| Inline code, code blocks | `` `code` ``, fenced ` ``` ` blocks with a language tag for syntax highlighting |
| Lists | `-` bullets, `1.` numbered (explicit numbers are kept), nesting by indentation |
| Task lists | `- [ ]` open, `- [x]` done |
| Tables | pipe tables with `:---`, `:---:`, `---:` column alignment |
| Block quotes | `>` prefixed lines |
| Callouts | `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`, `[!CAUTION]` |
| Footnotes | `[^label]` references with `[^label]: text` definitions, in any order |
| Links | `[text](url)`, bare URLs and `www.` hosts autolink, `<mail@example.org>` |
| Note links | ordinary links targeting a note's filename — see [Linking between notes](#linking-between-notes) |
| Images | `![alt](path)` relative to the note |
| Horizontal rules | `---` on its own line |

Deliberately not supported: math (`$x^2$` stays literal text), definition
lists, and emoji shortcodes (`:smile:` stays text). Raw HTML renders on
GitHub but is dropped with a warning when rendering to PDF, and remote
image URLs become links there since PDF rendering never touches the
network.

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
just site-check  # rebuild the site frontend with Bun, check, lint, and test it (the CI gate)
```

The browser-side code of the site export lives in `site/` and is built
with [Bun](https://bun.sh); `just site-build` writes the result to
`src/site/dist/`, which is committed. `cargo build` never runs Bun.

Tests use [`insta`](https://insta.rs) snapshots across all layers (ADR 0021).
When a change alters output, the snapshot assertions fail and write `.pending-snap`
files; review and accept them with [`cargo-insta`](https://insta.rs/docs/cli/):

```bash
cargo insta review   # interactively accept/reject pending snapshots
cargo insta accept   # accept all pending snapshots
```

## Project page

The landing page is generated from `docs/pages/` by the
[project-page-starter](https://github.com/jakobwesthoff/project-page-starter)
generator. A push to `main` that touches `README.md` or `docs/pages/` rebuilds
and deploys it through
[`.github/workflows/pages.yml`](https://github.com/jakobwesthoff/ntropy/blob/main/.github/workflows/pages.yml).
To build it manually (the generator runs on [Bun](https://bun.sh)):

All commands run from this repo's root:

```bash
# One-time: clone the generator beside this repo and install its dependencies
git clone https://github.com/jakobwesthoff/project-page-starter.git ../project-page-starter
(cd ../project-page-starter/generator && bun install)

# Build docs/pages into ./dist
bun run ../project-page-starter/generator/bin/generate.ts \
  --docs ./docs/pages \
  --readme ./README.md \
  --output ./dist \
  --templates ../project-page-starter/templates

# Preview locally, then open the printed URL
bunx serve dist
```

The demo recording in `docs/pages/assets/` is produced by `docs/vhs/record.sh`
(see [`docs/vhs/README.md`](https://github.com/jakobwesthoff/ntropy/blob/main/docs/vhs/README.md)).

## Design

The full design is recorded as decision records under
[`docs/adr/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/adr)
and narrative documents under
[`docs/design/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/design).

## License

ntropy is licensed under the Mozilla Public License 2.0. See [`LICENSE`](https://github.com/jakobwesthoff/ntropy/blob/main/LICENSE).

Copyright (c) 2026 Jakob Westhoff <jakob@westhoffswelt.de>
