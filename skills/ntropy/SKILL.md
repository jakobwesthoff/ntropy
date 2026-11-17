---
name: ntropy
description: >-
  Work with ntropy, a Markdown note-taking CLI whose notes live in a vault: create, search, edit and delete notes, keep views, render a note to PDF, HTML or terminal text, export a vault as a static website, write site or document themes, work in an encrypted vault, and script against any of it. Use for any task involving ntropy, a note vault, .ntropy-vault files or `ntropy` commands, and read this first: ntropy blocks on a terminal unless it is run with `-n`.
metadata:
  tags: ntropy, notes, markdown, vault, cli, site, website, html
  ntropy-version: "2.14.0"
---

Read this file to its last line before you run ntropy. The last line is
`End of the ntropy skill.` If you have not seen it, read the file again in
full, from its path or from the output of `ntropy skill`. Read a file under
`references/` when a step below names it. Each ends with its own end line
too.

Eight files hold details for some tasks:

- [references/writing-notes.md](references/writing-notes.md), or `ntropy skill writing-notes`: before you
  author a note beyond the flow below: frontmatter rules, tags, links,
  templates, daily notes, titles and slugs.
- [references/querying.md](references/querying.md), or `ntropy skill querying`: before a search
  beyond one tag or word: the query language, output parsing, exit codes,
  deleting.
- [references/vaults.md](references/vaults.md), or `ntropy skill vaults`: to create or locate a
  vault, and for pointer files, encryption, the `vault` commands, git and
  configuration.
- [references/views.md](references/views.md), or `ntropy skill views`: before you add or change a
  view.
- [references/rendering.md](references/rendering.md), or `ntropy skill rendering`: before `render` or
  `show`: formats, where the artifact goes, themes, paper, linked sets.
- [references/site.md](references/site.md), or `ntropy skill site`: before `ntropy site`: the
  website export, its `[site]` table and navigation.
- [references/site-themes.md](references/site-themes.md), or `ntropy skill site-themes`: before you
  write or adapt a site theme.
- [references/document-themes.md](references/document-themes.md), or `ntropy skill document-themes`: before
  you write or adapt a document theme for PDF and Typst output.

# Working with ntropy

ntropy files notes by metadata, not folders. A **vault** is a plain
directory; the notes in its `all-notes/` are the whole database, with no
index and no hidden state. A note is a Markdown file named
`<ulid>-<slug>.md` with YAML frontmatter. Its 26-character ULID is its
identity and `title` is required. Every frontmatter field can be queried, and
materialized symlink **views** make a field browsable as folders.

## Rules for every task

1. **Pass `-n` on every command**, and `--print` (`-p`) or `--id` on `new`
   and `today`. Without them ntropy opens a fuzzy picker or `$VISUAL` and
   `$EDITOR` and blocks whenever a controlling terminal exists. Capturing the
   output does not prevent that, so `$(ntropy search …)` without `-n` hangs.
   Put `--` before a title or selector that may start with `-`, which clap
   would read as a flag: `ntropy new -n -p -- "$title"`.
2. **Know which vault you hit.** `ntropy info` names the active vault and the
   rule that chose it: `--vault <path>`, then `$NTROPY_VAULT`, then the
   nearest ancestor of the current directory holding a `.ntropy-vault` pointer
   file or a `.ntropy/` directory, then the global default. Pin it with
   `--vault <path>` when it matters. If `info` shows no vault or an unexpected
   one, stop and ask the user which vault to use. Never run `ntropy init` or
   `init --set-default` on your own initiative: `--set-default` rewrites the
   user's global config. A note made in the wrong vault is removed with
   `delete -n -f <ulid>` under that vault's `--vault`.
3. **NEVER hand-create files in `all-notes/`.** You would have to invent a
   ULID, and a wrong one corrupts note identity. ntropy allocates the
   identity, the location and the filename; the content is yours.
4. **Author with `new --empty -p`, then `write` the whole note.**
   `ntropy new -n --empty -p -- <title…>` prints the path of an empty file,
   and `ntropy write -n <path|ulid|filename>` replaces the note with the text
   on stdin. Nothing is read back, no template is in the way, no `reconcile`
   follows, and it works the same in an encrypted vault:

   ```bash
   note=$(ntropy new -n --empty -p -- "Quarterly review")
   ntropy write -n "$note" <<'EOF'
   ---
   title: Quarterly review
   tags: [work, planning]
   status: draft
   ---
   # Quarterly review

   Body in ordinary Markdown.
   EOF
   ```

   What you pipe becomes the file, so it must be a complete, well-formed
   note: a frontmatter block first (`---` … `---`) with `title` (the one you
   passed to `new`), `tags` a flat list of strings if present, never `id`,
   `created` or `modified`, and every value that contains `: ` or starts with
   YAML syntax quoted. A text that is no note is refused and nothing is
   written, so fix it and `write` the same path again. Write in the same step
   you create: until then the `--empty` file is malformed, every command
   warns about it, `--strict` fails on it and no search returns it. `delete`
   does not see a malformed note either, so to drop an abandoned one `write` a
   minimal note (`---` / `title: x` / `---`) and then delete it. To rewrite a
   note, read it with `search -n -P <ulid>`, change it, and `write` the whole
   file back.
5. **Run `ntropy reconcile` after editing a note file directly**, for example
   one you started with `new -n -p -t <name>`. The filename slug and the views
   drift from the file until you do. `write` and ntropy's own editor realign
   both themselves, so never reconcile after them, except to move a retitled
   note's asset folder, which only reconcile renames. Reconcile is cheap and
   idempotent.
6. **Retitle by editing the frontmatter `title`, then reconcile; never rename
   the file.** Reconcile renames the file to the new slug, keeps the ULID,
   rewrites the slug in links of other notes and renames the note's asset
   folder `all-notes/_assets/<file name>/` with the links into it.
7. **Link by filename**: `[Title](<ulid>-<slug>.md)`, taking the filename from
   `ntropy search -n -p <query>`. Never link by title or by a path into a
   view. A link survives retitles because it is recognized by its ULID.
8. **Read with `search -n`**: no selector lists every note; a selector is a
   full ULID, a note's filename or path, or a query (`tag:work and not
   status:done`; two words side by side must both occur, one quoted argument is
   a phrase). `-p` prints paths, `--id` ULIDs and `-P` one note's exact text,
   each one per line and nothing else. `search` exits 1 when nothing matches,
   and also on every error and on every warning under `--strict`, where
   matching notes still print. Tell them apart by stderr (`No notes matched`,
   `error:`, `warning:`), never by the code alone.
9. **Delete one note by its ULID**: search first, confirm the single ULID,
   then `delete -n -f <ulid>`; `-f` is required without a prompt, and a broad
   query is never a selector for it. An empty selector is refused.
10. **A note file that carries another note's ULID, such as a copy,** raises
    a warning, and a ULID selector names both files, so the commands that take
    one note (`-P`, `write`, `delete`) refuse it as ambiguous. Run
    `ntropy reconcile`, which gives the copy its own ULID, or name the file.
11. **Derived state is not yours.** NEVER write inside a view directory
    (`by-*/`): its leaves are symlinks that the next sync prunes. Do not
    commit view directories, do not edit ntropy's managed `.gitignore`
    entries, and create no file or directory named `all-notes`, `.ntropy`,
    `.gitignore`, `README.md` or `.ntropy-vault` yourself.
12. **Displayed text is escaped.** Control and bidi characters in titles,
    names and paths show as `\n`, `\u{1b}`, so a table cell can differ from the
    file. Use `-P` for exact text. A `-p` path is exact; a note whose file name
    would need escaping has none, and `-p` fails or leaves it out with a
    warning until `ntropy reconcile` renames it.
13. **`site` is a reserved frontmatter key.** A note's `site` table shapes the
    exported website's navigation and picks the theme template for the note,
    and is hidden from the page. Never put content under it and never invent
    keys in it.
14. **Encrypted vaults** (marked by `.ntropy/identity.pub`, notes stored as
    `all-notes/<ulid>.age`): `new` and `write` work while locked; reading,
    `today`, `render`, `show` and `site` need the key. Never read a `-p` path,
    it is ciphertext: use `search -n -P <ulid>`. Views are disabled, so `view
    add` is refused. Rendered and exported output is plaintext: write it
    outside the vault. Headless, `--passphrase-file <file>` (the first line is
    the passphrase) is enough; `--identity <file>` or `$NTROPY_IDENTITY` names
    an unwrapped age secret key, never `.ntropy/identity.age`. With `-n`
    nothing prompts. Details: `references/vaults.md`, or `ntropy skill vaults`.
15. **`render` writes into the current directory** unless you pass `-o`, and
    the default PDF needs `typst` on `PATH`. Read `ntropy skill rendering`
    before a render and `ntropy skill site` before an export.

## Command reference

Global flags on every command: `--vault <path>`, `-n`/`--non-interactive`,
`--strict` (malformed notes and skipped views become errors instead of
warnings), `-i`/`--identity <path>` and `--passphrase-file <path>` for
encrypted vaults.

| Command | Purpose |
|---------|---------|
| `ntropy init [path]` | Scaffold or complete a vault; idempotent. `--encrypted` makes an encrypted one. Rule 2 says when not to. |
| `ntropy new -n -p <title…>` | Create a note from a template and print its path; `-t <name>` picks `.ntropy/templates/<name>.md`; `--empty` creates it with no content (conflicts with `-t`); `--id` prints the ULID instead of the path. |
| `ntropy today -n -p` | Print today's daily note path, creating it on first use each day; `--id` prints the ULID. |
| `ntropy write <id\|filename\|path>` | Replace one note with the whole text on stdin, realign its filename and refresh views. It names its target, never searches. Refuses text that is not a well-formed note. Prints the path. |
| `ntropy search -n [selector…]` | List notes as a plain table, newest first (alias `list`); `-p`, `--id`, `-P` as in rule 8. Query language: [references/querying.md](references/querying.md), or `ntropy skill querying`. |
| `ntropy delete -n -f <id>` | Delete one note, its asset folder (kept while a copy carries the ULID or another note links it; `--remove-assets` removes it past a link only) and refresh views. |
| `ntropy render -n -p <id>` | Render one note: `--to pdf` (default), `typst`, `html` or `ansi`; `-o` names the artifact, `--force` replaces one, `--theme` overrides the theme. Read `ntropy skill rendering`. |
| `ntropy show -n <id>` | Print one note drawn for a terminal; into a pipe it is plain text, so read exact text with `search -n -P`. |
| `ntropy site -n -o <dir> [query]` | Export the vault, or the notes a query selects, as a static website. A non-empty `<dir>` is refused without `--force`; `-p` prints the front page's path; `site theme init <name>` copies the built-in theme into the vault; `--strict` fails on its warnings. Read `ntropy skill site`. |
| `ntropy asset add -n <id> <file…>` | Copy (`--move`) files into the note's asset folder `all-notes/_assets/<note file name>/` and print their Markdown links; `asset list -n [--path | --folder] <id>` lists them (`--path` prints each file). Read `ntropy skill writing-notes`. |
| `ntropy reconcile` | Give copied notes their own ULIDs, realign drifted filenames, refresh links, rename asset folders with their notes and warn about broken or unmanaged asset links (`--strict` fails on them), re-sync views and `.gitignore`. |
| `ntropy view list\|add\|remove` | Manage materialized views, e.g. `view add by-status --field status`. Read `ntropy skill views`. |
| `ntropy tags -n` | Every tag with its note count; check it before inventing a tag. |
| `ntropy info` | The active vault and how it resolved, the global default, vault statistics; `-p` prints the vault's absolute path alone. |
| `ntropy unlock` / `ntropy lock` | Store or forget an encrypted vault's key. |
| `ntropy vault encrypt\|decrypt\|rekey\|passphrase` | Convert a vault's storage or manage its key; they rewrite every note and ask first (`-y` skips the question, `--resume` finishes an interrupted run). Read `ntropy skill vaults`. |
| `ntropy lsp` | Language server for editors; not used from scripts. |
| `ntropy skill [part]` | Print this skill or one of its parts; `--install <dir>` writes it to `<dir>/ntropy/`, replacing its `references/`. It needs no vault. |

End of the ntropy skill.
