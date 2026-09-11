# ntropy

An opinionated Markdown note-taking and management CLI where metadata, not
folders, is the filing system. No database, no proprietary app, no folder
hierarchy to maintain by hand: plain Markdown files and their frontmatter.

The short version: write Markdown, tag it, and let ntropy do the filing. Fuzzy
full-text search, a query language, browsable views materialized straight into
your filesystem, editor-native link and tag completion over LSP, PDF and
website export, and no database pretending to be a note app.

The documentation lives at <https://ntropy.westhoffswelt.de>; this file is
the entry point: install, first steps, and how to work on ntropy itself.

## Why I built this

I live on the command line and in Neovim, and every note app I tried wanted me
back inside its own (usually graphical) UI to make sense of files it nominally
stored as plain text. I wanted the inverse: notes that are *just* Markdown
files, a CLI to manage them, and nothing stateful in between. ntropy is the
heavily opinionated result. Notes live flat in one vault, a note's identity is
a stable ULID rather than its title, and any hierarchy you browse is a derived
projection of the frontmatter instead of the canonical storage. Switching vaults
is cheap, one for work, one for private, or a per-project vault pinned by a
`.ntropy-vault` file in a repo, so documenting a project is the same motion as
any other note. It scratched my itch; maybe it scratches yours.

## Installation

```bash
cargo install ntropy
```

### Pre-built binaries

Pre-built binaries are available on the
[GitHub Releases](https://github.com/jakobwesthoff/ntropy/releases) page for
macOS (Apple Silicon & Intel) and Linux (x86_64 & aarch64, statically linked).

> [!NOTE]
> ntropy supports macOS and Linux. Windows is not supported; see
> [Limitations](https://ntropy.westhoffswelt.de/notes/limitations.html).

## Quick start

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

New notes are stamped out from
[templates](https://ntropy.westhoffswelt.de/notes/templates-and-daily-notes.html);
`today` has its own daily template; and `search` speaks a small
[query language](https://ntropy.westhoffswelt.de/notes/query-language.html),
opening an
[interactive picker](https://ntropy.westhoffswelt.de/notes/the-interactive-picker.html)
when more than one note matches.

You never have to tell ntropy which vault you mean from inside one; it
[finds it for you](https://ntropy.westhoffswelt.de/notes/finding-the-vault.html).

## Documentation

The full documentation is a website that ntropy exports from the vault under
[`docs/website/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/website)
in this repository, so it reads the same as any vault you publish yourself:

- [Getting started](https://ntropy.westhoffswelt.de/tags/docs/start/index.html):
  installation, first steps, a tour, and the command reference.
- [Notes and the vault](https://ntropy.westhoffswelt.de/tags/docs/notes/index.html):
  the note format, Markdown, templates, finding the vault, views, encryption,
  and configuration.
- [Finding notes](https://ntropy.westhoffswelt.de/tags/docs/find/index.html):
  the query language and the interactive picker.
- [Publishing](https://ntropy.westhoffswelt.de/tags/docs/publish/index.html):
  rendering to PDF and HTML, document themes, exporting a website, and site
  themes with their templates.
- [Integrations](https://ntropy.westhoffswelt.de/tags/docs/integrate/index.html):
  the language server, scripting and the shell, and the agent skill.
- [Development](https://ntropy.westhoffswelt.de/tags/docs/develop/index.html):
  building ntropy, its limitations, and the design records.

The design itself is recorded as decision records under
[`docs/adr/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/adr)
and narrative documents under
[`docs/design/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/design).

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
just pages     # export the documentation vault as the project page into ./dist
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

The project page at <https://ntropy.westhoffswelt.de> is the documentation
vault under `docs/website/`, exported by ntropy itself. A push to `main` that
touches `docs/website/` rebuilds and deploys it through
[`.github/workflows/pages.yml`](https://github.com/jakobwesthoff/ntropy/blob/main/.github/workflows/pages.yml),
which exports the vault with the newest released binary. To build it with
your working copy:

```bash
just pages           # exports docs/website into ./dist
open dist/index.html
```

The vault is an ordinary ntropy vault: `ntropy --vault docs/website new`,
`search`, and `write` work on it, and its `.ntropy/config.toml` holds the
site's title, front page, and sidebar root. The demo recording in
`docs/website/assets/` is produced by `docs/vhs/record.sh`
(see [`docs/vhs/README.md`](https://github.com/jakobwesthoff/ntropy/blob/main/docs/vhs/README.md)).

## License

ntropy is licensed under the Mozilla Public License 2.0. See [`LICENSE`](https://github.com/jakobwesthoff/ntropy/blob/main/LICENSE).

Copyright (c) 2026 Jakob Westhoff <jakob@westhoffswelt.de>
