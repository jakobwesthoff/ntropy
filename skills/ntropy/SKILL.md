---
name: ntropy
description: >-
  Create, search, edit, delete, and render notes to PDF or HTML in ntropy
  Markdown vaults; export a vault or a subset as a static website with a
  configurable sidebar and theme; create and manage vaults, views, and
  templates. Use for any task involving ntropy, a note vault, or
  .ntropy-vault files.
metadata:
  tags: ntropy, notes, markdown, vault, cli, site, website, html
---

# Working with ntropy

ntropy is a Markdown note-taking CLI where metadata, not folders, is the filing
system. A **vault** is a plain directory; the notes in its `all-notes/`
subdirectory are the entire database (no index, no hidden state). A note is a
Markdown file named `<ulid>-<slug>.md` with YAML frontmatter; the 26-character
ULID is its identity, `title` is required, and every frontmatter field is
instantly filterable and browsable. Organization is derived on demand: a query
language for filtering, and materialized symlink views for filesystem browsing.

## Golden rules for agents

1. **ALWAYS run non-interactively.** Pass `-n` on every command and
   `--print` on `new`/`today`. Without them ntropy opens an interactive fuzzy
   picker or the user's `$VISUAL`/`$EDITOR` and blocks whenever a controlling
   terminal exists — piping or capturing the output does NOT prevent that, so
   `$(ntropy search …)` without `-n` hangs waiting for keys.
2. **NEVER hand-create files in `all-notes/`.** You would have to invent a
   ULID, and a wrong or duplicate one corrupts note identity. ntropy allocates
   the identity, the location and the filename; only the content is yours.
3. **Author a note with `new --empty` then `write`.** This is the path to
   prefer whenever you already know what the note should say. `ntropy new
   --empty -p <title>` prints the path of an empty file; `ntropy write <path>`
   stores the text you feed it on stdin. Nothing is read back, no template gets
   in the way, and it works the same in an encrypted vault. `write` realigns the
   filename and refreshes the views itself, so no `reconcile` is needed after
   it.
4. **A file left by `--empty` is malformed until you write it.** Every ntropy
   command warns about it, `--strict` fails on it, and it is in no search
   result. Always `write` it in the same step you created it. What you write
   must be a complete, well-formed note: see rule 5.
5. **`write` takes a whole note, not a fragment.** Frontmatter block first with
   at least `title:` (use the title you passed to `new`, since the filename slug
   came from it), `tags:` as a flat list of strings if present, never `id:` or
   dates, then the Markdown body. Anything that is not a well-formed note is
   refused and nothing is written.
6. **Run `ntropy reconcile` after editing a note file directly.** Editing the
   file yourself instead of using `write` leaves the filename slug and the views
   stale; reconcile realigns filenames, refreshes inter-note links, and re-syncs
   views. It is cheap and idempotent. `write` does this for you.
7. **Check the active vault before mutating.** `ntropy info` names the vault
   and the rule that resolved it. When in doubt, pin the vault explicitly with
   `--vault <path>`.
8. **NEVER touch derived state.** Do not write inside `by-*/` view
   directories, do not edit ntropy's managed `.gitignore` entries, and do not
   store `id` or dates in frontmatter (identity lives in the filename).
9. **Delete by ULID, with `-f`.** `delete` requires exactly one match and, in
   non-interactive mode, `--force`. Search first, then
   `ntropy delete -n -f <ulid>`.
10. **`render` to PDF needs `typst` on `PATH`; `typst` and `html` need no
   tool.** It resolves to exactly one note; pass `-p` to capture the
   artifact path (`out=$(ntropy render -n -p <ulid>)`). `--to typst` emits
   the Typst document. `--to html` writes the note as a web page,
   `<stem>.html` plus a `<stem>_files/` directory beside it that the page
   needs; move the two together ([references/site.md](references/site.md)).
11. **The vault's theme is automatic; never pass `--theme` to get it.** Which
   theme depends on the format: `pdf` and `typst` use `[render] theme` from
   `.ntropy/themes/typst/`, `html` and `site` use `[site] theme`, a
   directory in `.ntropy/themes/site/`. `--theme <name>` picks a different
   one, `--theme default` forces ntropy's built-in look, and `site theme
   init <name>` copies the built-in site theme out as a starting point. A
   theme that is missing or broken fails the render rather than falling
   back.
12. **Rendering a linked set: no `-o`, one directory.** A note link becomes a
   link to `<target-slug>.pdf` (or `.html`), the name `render` gives the
   target's own artifact by default. Render each note from the same working
   directory without `-o` and the cross-references find each other; rename
   an artifact and its incoming links break. Rendering again over an
   existing artifact needs `--force`; `render` never overwrites silently.
13. **`site` is a reserved frontmatter key.** A note's `site` table shapes
   the exported website's navigation (`order`, `label`, `hidden`, `index`,
   `listing`, `related`) and picks the theme template that renders the note
   (`template`); it is hidden from the page. Never put content under it,
   and never invent keys in it.
14. **The site config names notes by ULID.** `[site] index` and every
   `{ note = … }` item of a `[[site.nav]]` table take the full 26-character
   ULID, never a title, slug, or path. Look it up with `ntropy search -n`
   first.

## Do / don't

| DON'T | DO |
|-------|----|
| `ntropy new My note` — blocks in an editor | `ntropy new --print My note` |
| Write a new file into `all-notes/` yourself | `path=$(ntropy new --empty -p …)`, then `ntropy write "$path"` |
| `ntropy new --empty -p …` and move on | `write` it in the same step; an unwritten note is malformed |
| Feed `write` a body with no frontmatter | feed it the whole note, frontmatter block first |
| `--empty --template meeting` — contradictory, refused | pick one: a template stamps content, `--empty` writes none |
| Write into an encrypted vault's `-p` path | `ntropy write <id>` — the only way to author there |
| `ntropy reconcile` after every `write` | nothing; `write` realigns and refreshes views itself |
| Rename a note file to retitle it | edit the frontmatter `title`, then `ntropy reconcile` |
| Put `id:` or `created:` in frontmatter | nothing — identity and date live in the filename ULID |
| `ntropy delete -n -f tag:old` — broad query | `ntropy delete -n -f <full-26-char-ulid>` |
| Create or edit files inside `by-*/` view directories | edit the canonical file in `all-notes/` |
| Link notes by title or view path | `[Title](<ulid>-<slug>.md)` |
| Edit a note's frontmatter and stop there | edit, then `ntropy reconcile` |
| `ntropy site -o ./public` into a used directory | `--force`, on purpose, or a fresh directory |
| `index = "Welcome"` in `[site]` | the note's ULID from `ntropy search -n` |

## Command reference

| Command | Purpose |
|---------|---------|
| `ntropy init [path]` | Scaffold or complete a vault; idempotent. `--set-default` records it as the global default. |
| `ntropy new --print <title…>` | Create a note from a template, print its path. `-t <name>` picks `.ntropy/templates/<name>.md`. `--empty` creates the file with no content instead, for when you write the whole note yourself (conflicts with `-t`). |
| `ntropy today --print` | Print today's daily note path, creating it on first use each day. |
| `ntropy write <id\|filename\|path>` | Replace that note's content with the whole note text read from stdin, then realign the filename and refresh views. Names its target, never searches: a full ULID, the filename, or the path. Refuses text that is not a well-formed note. Prints the resulting path. |
| `ntropy search -n [id\|query]` | List/filter notes as a plain table (alias `list`). No selector = all notes. Exits non-zero on no match. Add `-p` to print matching paths, one per line, instead of the table, or `-P` to print one note's text. |
| `ntropy delete -n -f <id>` | Delete one note and refresh views. |
| `ntropy render -n -p <id> -o out.pdf` | Render one note to a document. The vault's configured theme applies automatically; `--theme <name>` overrides it, `--theme default` forces the built-in look. `--to` picks the format: `pdf` (default, via ntropy's own typst engine, needing only `typst` on `PATH`), `typst` for the emitted document, or `html` for a web page with a `<stem>_files/` directory beside it; the last two need no external tool. Resolves to exactly one note; `-p` prints the artifact path; an existing artifact is refused without `--force`. |
| `ntropy site -n -o <dir> [query]` | Export the vault, or the notes a query selects, as a static website: a page per note, a tag tree, a page per view and group, a front page, sidebar, search, works from disk. A non-empty `<dir>` is refused without `--force`; `-p` prints the front page's path; `--theme` overrides the site theme. `site theme init <name>` copies the built-in theme into the vault. Warnings name missing files, dangling links, and unresolvable navigation; `--strict` fails on them. Details: [references/site.md](references/site.md). |
| `ntropy reconcile` | Realign drifted filenames, refresh links, re-sync views and `.gitignore`. |
| `ntropy view list\|add\|remove` | Manage materialized views, e.g. `view add by-status --field status`. |
| `ntropy tags -n` | Every tag with its note count — check this before inventing new tags. |
| `ntropy info` | Active vault + how it resolved, global default, vault statistics. `-p`/`--print` prints the vault's absolute path alone, for scripts. |
| `ntropy unlock` / `ntropy lock` | Store or forget an encrypted vault's key. |
| `ntropy vault encrypt\|decrypt\|rekey\|passphrase` | Convert a vault's storage or manage its key. Rewrites every note, so pass `-y` to skip the confirmation; `--resume` finishes an interrupted run. |
| `ntropy lsp` | Language server for editors (link/tag completion, go-to-definition); not used from scripts. Editor setup lives in the ntropy README. |

Global flags on every command: `--vault <path>`, `-n`/`--non-interactive`,
`--strict` (malformed notes become errors instead of skip-warnings),
`-i`/`--identity <path>` and `--passphrase-file <path>` for encrypted vaults.

## Encrypted vaults

A vault whose `.ntropy/identity.pub` exists stores its notes encrypted as
`all-notes/<ulid>.age`. Everything above works there unchanged once the vault
is unlocked; what differs for an agent:

- **Never read a `-p` path directly.** In an encrypted vault it is the
  ciphertext file. Use `ntropy search -n -P <id>` to get a note's text; it
  reads the same in either kind of vault.
- **Reading needs the key**, so a locked vault fails with a message naming
  `ntropy unlock`. Creating does not: `ntropy new` works locked. `ntropy today`
  does not, because it finds today's note by title.
- **Author with `write`, never by writing the path.** The `-p` path is an age
  container, so text written into it is not a note. `ntropy write <id>` stores
  content through the vault's cipher and is what makes an encrypted vault
  scriptable at all. `new --empty` plus `write` works here exactly as it does in
  a plaintext vault.
- **`write` works locked too**, like `new`: it resolves its target by name and
  reads no note, and encrypting needs only the public recipient.
- **Headless use** wants `--identity <path>` (or `$NTROPY_IDENTITY`) and
  `--passphrase-file <path>`; with `-n` ntropy never prompts.
- **Views do not exist** there, so `ntropy view add` is refused.
- **A rendered PDF is plaintext.** Write it outside the vault.

## Vault resolution (which vault will I hit?)

`--vault` > `$NTROPY_VAULT` > walk-up to the nearest ancestor with a
`.ntropy-vault` pointer file or `.ntropy/` directory (pointer wins) > the
global default. A `.ntropy-vault` file is one line pointing at a vault
elsewhere, which is how a project pins its own vault. Details and creation
recipes for global, project-local, and custom vaults:
[references/vaults.md](references/vaults.md).

## Core workflows

**Author a note (the path to prefer):** `new --empty` allocates the identity,
`write` supplies the content. Nothing is read back, and this is identical in a
plaintext and an encrypted vault:

```bash
path=$(ntropy new --empty -p Quarterly review)
ntropy write "$path" <<'EOF'
---
title: Quarterly review
tags: [work, planning]
status: draft
---
# Quarterly review

Body in ordinary Markdown.
EOF
```

No `ntropy reconcile` afterwards: `write` realigns the filename and refreshes
views itself. What you feed it has to be a well-formed note, since nothing
stamped a skeleton for you: a YAML frontmatter block first, `title` in it (the
title you passed to `new`, so the filename slug matches), `tags` a flat list of
strings if present, no `id`/`created`/`modified` fields, YAML-quoting for values
with `: ` in them, then the Markdown body. Text that is not a well-formed note
is refused and nothing is written.

**Rewrite an existing note:** read it out, transform, write it back. `-P` and
`write` are inverses and both read the same in either kind of vault:

```bash
ntropy search -n -P 01KWVBW61WHJY7K27WNETSF641 > /tmp/note.md
# edit /tmp/note.md
ntropy write 01KWVBW61WHJY7K27WNETSF641 < /tmp/note.md
```

**Start from a template instead:** when you want the vault's own skeleton, for
example a meeting note whose shape the user defined:

```bash
path=$(ntropy new --print -t meeting Standup)
# edit "$path" directly, then:
ntropy reconcile
```

Frontmatter rules in full, plus tags, inter-note links, and template authoring:
[references/writing-notes.md](references/writing-notes.md).

**Find and read notes:**

```bash
ntropy search -n 'tag:work and not status:done'    # table: ID DATE TITLE TAGS PATH
ntropy search -n 01KWVBW61WHJY7K27WNETSF641        # one note by ULID
path=$(ntropy search -n -p 01KWVBW61WHJY7K27WNETSF641)   # just the file path
```

Full query language (`tag:`, `field:`, `text:`, `and`/`or`/`not`), output
parsing, and exit-code recipes:
[references/querying.md](references/querying.md).

**Set up a project vault:**

```bash
ntropy init myproject/notes
echo "notes" > myproject/.ntropy-vault   # commands anywhere in the project now hit it
```

**Make a dimension browsable:**

```bash
ntropy view add by-status --field status
```

View semantics, drift, and git rules: [references/views.md](references/views.md).

**Publish a documentation subtree as a website:** tag the pages as a tree,
give each section a landing note, order the pages through their `site`
table, root the sidebar at the tree, and export:

```bash
path=$(ntropy new --empty -p Welcome)
ntropy write "$path" <<'EOF'
---
title: Welcome
tags: [handbook/start]
site: { index: true, label: Getting Started, order: 1 }
---
# Welcome

Where to begin.
EOF
# .ntropy/config.toml: [site] root = "tags/handbook", related = false,
# index = "<ulid of the front page note>"
ntropy site -n -o ./public --strict tag:handbook
```

The `[site]` table, the nav table, the `site` frontmatter keys, and the
standalone `render --to html` page: [references/site.md](references/site.md).
Writing a theme (layout, tokens, fonts, icons, markup):
[references/site-themes.md](references/site-themes.md).
