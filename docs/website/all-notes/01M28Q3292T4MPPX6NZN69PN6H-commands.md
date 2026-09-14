---
title: Commands
tags: [docs/start]
site:
  order: 2
---
Every ntropy command in one table, with the flags that matter, followed by the
flags every command accepts. `ntropy <command> --help` prints the full option
list of a command; a bare `ntropy` prints the overview.

## Commands

| Command | What it does |
|---------|--------------|
| `init [path]` | Scaffold (or complete) a vault; idempotent. The target is `path` or, if omitted, `--vault`; passing both is an error, and neither uses the current directory. `--set-default` records the vault as the global default. `--encrypted` creates an [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md). |
| `new <title>` | Create a note from a [template](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md) and open it. `--template`/`-t <name>` picks a template; `--empty` writes no content at all, for a caller that authors the note itself; `--print`/`-p` just prints the path. |
| `today` | Open today's note, creating it from the [`today` template](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md) on first use that day. `--print`/`-p` just prints the path. |
| `write <id\|filename\|path>` | Replace one note's content with text read from stdin, then realign the filename and refresh the views. Names its target rather than searching for it, never prompts, and works on an encrypted vault, including a locked one. |
| `search [id\|query]` | The one browse/filter/full-text/open entry point (alias `list`). Takes a [query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) expression and opens the [picker](01M28Q32FZCNHV2PZ5FSG8E360-the-interactive-picker.md) when several notes match; with `-n` it prints the matches as a table instead. `--print`/`-p` prints the selected note's path instead of opening it; `--print-content`/`-P` prints the note's text (exactly one note). |
| `delete <id\|query>` | Remove a note and refresh the views (`-f`/`--force` skips the prompt). Must resolve to exactly one note: several matches open the picker interactively and are an error under `-n`, where `--force` is required as there is no prompt. |
| `render [id\|query]` | [Render one note to a PDF](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md) with ntropy's own typst engine (only `typst` required on `PATH`), or with `--to html` to a web page with a `<stem>_files/` directory beside it, needing no tool. Must resolve to exactly one note; with no selector the picker opens over all notes, like `search`. `--to` picks the format (default `pdf`, or `typst` for the emitted Typst document), `--engine` overrides the format's default engine, `-o` the output path (default `./<slug>.<ext>`), `--theme` overrides the vault's [configured theme](01M28Q32HRVNF9EBZTC69XDZ49-document-themes.md), `-p` prints the artifact path. An existing artifact is refused unless `--force` replaces it. |
| `site -o <dir> [query]` | [Export the vault as a static website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md): a page per note, a tag tree, a page per view and group, sidebar, outline, breadcrumbs. Works from disk without a server. The optional query restricts the exported notes. `--force` empties a non-empty directory, `--theme` overrides the [site theme](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md), `-p` prints the front page's path. `site theme init <name>` copies the built-in theme into the vault. |
| `reconcile` | Catch up after edits made outside ntropy: realign filenames whose slug drifted from the title, rewrite links that pointed at the old names, re-sync every view, and bring `.gitignore` in line with the configured views. |
| `view list\|add\|remove` | Manage [materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md), e.g. `ntropy view add by-status --field status`. Removing a view leaves its directory on disk and tells you so; ntropy never deletes a directory. |
| `tags` | List every tag with its note count. |
| `info` | Show the active vault and how it was resolved, the global default, and stats: note/tag/view/template counts, skipped-note warnings, the creation-date span, the top tags, and the template names. `--print`/`-p` prints the vault's path alone, for [shell integration](01M28Q32N56MS70ETPES4794HJ-scripting-and-the-shell.md). |
| `unlock` / `lock` | Store or forget an [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md)'s key, so ordinary commands need no passphrase. |
| `vault encrypt\|decrypt\|rekey\|passphrase` | Convert a vault's storage, re-encrypt it to a fresh key, or change the passphrase. `encrypt`, `decrypt`, and `rekey` rewrite every note, so each asks first (`-y` skips) and each is safe to interrupt (`--resume` finishes). `passphrase` re-wraps only the key file and leaves the notes untouched. `rekey` and `passphrase` take `--new-passphrase-file` for the new passphrase. |
| `lsp` | Run the [language server](01M28Q32MG1WQ0BTSBGCER8A2R-language-server.md) over stdin/stdout for your editor. |

## Global flags

These work on any command:

| Flag | What it does |
|------|--------------|
| `--vault <path>` | Operate on the vault at this path. Overrides every other way of [finding the vault](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md). |
| `-n`, `--non-interactive` | Force plain behaviour even on a terminal: no picker, no editor, no prompts. Environments without a controlling terminal, such as cron and CI, get this automatically. |
| `--strict` | Treat malformed or badly named notes as errors instead of warnings. |
| `-i`, `--identity <path>` | Use this age identity file instead of the OS credential store. `$NTROPY_IDENTITY` is consulted when the flag is absent. See [encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md). |
| `--passphrase-file <path>` | Read the vault passphrase from the first line of this file wherever one would otherwise be typed. On `vault passphrase` it is the current passphrase. |

`-V`/`--version` prints the version and `-h`/`--help` the help.
