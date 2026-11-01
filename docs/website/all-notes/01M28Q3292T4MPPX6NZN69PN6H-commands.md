---
title: Commands
tags: [docs/reference]
site:
  index: true
  label: Reference
  order: 7
---
The reference starts with every command on one page: what it does and
the flags that matter, then the flags every command accepts.
`ntropy <command> --help` lists all options of a command, and a bare
`ntropy` prints the overview.

## Commands

| Command | What it does |
|---------|--------------|
| `init [path]` | Create a vault at `path`, or add what an existing vault lacks; it never touches what exists. Without `path` it uses `--vault`, or else the current directory; passing both is an error. `--set-default` records the vault as the global default. `--encrypted` creates an encrypted vault, see [Encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md). |
| `new <title>` | Create a note from a template and open it in your editor. `-t`/`--template <name>` picks the template. `--empty` creates the file with no content, for a script that writes the note itself. `-p`/`--print` prints the path instead of opening the editor. See [Templates and daily notes](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md). |
| `today` | Open today's note, created from the `today` template on first use that day. `-p`/`--print` prints the path instead. |
| `write <id\|filename\|path>` | Replace one note's whole text with what stdin holds, then realign the filename and refresh the views. It names its target, so it never searches or prompts. It works in an encrypted vault, a locked one included. |
| `search [id\|query]` | List, filter, full-text search and open notes; `list` is an alias. It takes a query or a full ULID. One match opens in your editor, several open the picker. Under `-n` the matches print as a table. `-p`/`--print` prints the chosen note's path instead, and `-P`/`--print-content` its text, for exactly one note. See [Finding notes](01M28Q32ESBVS3H2KJMRS99K87-finding-notes.md) and [Query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md). |
| `delete <id\|query>` | Delete exactly one note and refresh the views. It asks first; `-f`/`--force` skips the question, and under `-n` it is required. Several matches open the picker, and under `-n` they are an error. |
| `render [id\|query]` | Render one note, chosen as `search` chooses. `--to` picks the format: `pdf` (the default, needs `typst` on `PATH`), `typst`, `html` (with a `<stem>_files/` directory beside the page) or `ansi`. `-o` names the artifact, by default `./<slug>.<ext>`. `--force` replaces an existing artifact, and `-p` prints its path. `--theme` overrides the vault's theme, `--engine` the format's engine. See [Rendering to PDF and HTML](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md). |
| `show [id\|query]` | Print one note drawn for the terminal, chosen as `render` chooses, at the terminal's width (`--width` sets another). A note taller than the screen opens in `$PAGER`, or `less -R`; `-n` never pages. Into a pipe it is plain text. |
| `site -o <dir> [query]` | Export the vault as a static website that works from disk. A query restricts the exported notes. `--force` empties a non-empty `<dir>`, `--theme` overrides the site theme, and `-p` prints the front page's path. `site theme init <name>` copies the built-in theme into the vault. See [Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md). |
| `reconcile` | Catch up after editing files outside ntropy. It gives a copied note a ULID of its own, renames files whose slug no longer matches the title, and rewrites links to the old names. Then it re-syncs the views and brings `.gitignore` in line with them. |
| `view list\|add\|remove` | Manage views, as in `ntropy view add by-status --field status`. `remove` leaves the view's directory on disk. See [Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md). |
| `tags` | List every tag with its note count. |
| `info` | Show the active vault and how it was found, the global default, and statistics of the vault. `-p`/`--print` prints the vault's path alone. |
| `unlock` / `lock` | Store or forget an encrypted vault's key, so ordinary commands need no passphrase. |
| `vault encrypt\|decrypt\|rekey\|passphrase` | Convert a vault to encrypted storage and back, re-encrypt it to a fresh key, or change the passphrase. The first three rewrite every note. They ask first; `-y` skips the question, and under `-n` it is required. `--resume` finishes an interrupted run. `rekey` and `passphrase` take `--new-passphrase-file`. |
| `lsp` | Run the language server for your editor, over stdin and stdout. See [Language server](01M28Q32MG1WQ0BTSBGCER8A2R-language-server.md). |

## Global flags

These work on any command:

| Flag | What it does |
|------|--------------|
| `--vault <path>` | Operate on the vault at this path. It overrides every other way of finding the vault, see [Finding the vault](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md). |
| `-n`, `--non-interactive` | Plain behaviour even on a terminal: no picker, no editor, no prompts. Without a controlling terminal, as under cron and CI, this is automatic. |
| `--strict` | Treat malformed or badly named notes, and views skipped for an invalid name, as errors instead of warnings. |
| `-i`, `--identity <path>` | Use this age identity file instead of the OS credential store. Without the flag, `$NTROPY_IDENTITY` is read. |
| `--passphrase-file <path>` | Read the vault passphrase from the first line of this file wherever one would otherwise be typed. On `vault passphrase` it is the current passphrase. |

`-V`/`--version` prints the version and `-h`/`--help` the help.
