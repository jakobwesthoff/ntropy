---
title: ntropy
site:
  hidden: true
---
ntropy is a Markdown note-taking CLI where metadata, not folders, is the
filing system. Your notes stay plain Markdown files with frontmatter:
tagged, queryable, and materialized into directories you can browse. No
database, no proprietary app, no lock-in. Everything you write stays
legible to you, to your shell, and to any agent or model you point at it.

[Get started](01M28Q327TJQ73DX24WBSXZHPF-getting-started.md) or read the
source on [GitHub](https://github.com/jakobwesthoff/ntropy).

## What you get

- **Plain Markdown, no database.** No index, no cache, no hidden state.
  The files in your vault are the single source of truth: greppable,
  committable, and rebuildable at will. See
  [Notes and the vault](01M28Q329PPDQVGS1EMZ3WGE92-notes-and-the-vault.md).
- **A query language.** Filter by tag, frontmatter field, or full-text
  regex with `and`, `or`, and `not`, then pick from a fuzzy list when
  several notes match. See
  [Query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md).
- **Views that sync themselves.** Turn any frontmatter field into a
  directory of symlinks you can `cd` into, rebuilt after every change.
  See
  [Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md).
- **Documents and websites.** Render a note to PDF or to a standalone
  web page, or export the vault as a static site with navigation and
  search. See [Publishing](01M28Q32GMWCRTBEKY04WD1BQ0-publishing.md).

## See it work

[Watch the demo](../assets/demo.webm) (WebM), or the
[MP4 version](../assets/demo.mp4).

## Install

```bash
cargo install ntropy
```

Pre-built binaries for macOS (Apple Silicon and Intel) and Linux
(x86_64 and aarch64, statically linked) are on the
[GitHub Releases](https://github.com/jakobwesthoff/ntropy/releases)
page. To build from source:

```bash
git clone https://github.com/jakobwesthoff/ntropy.git
cd ntropy
cargo build --release
```

## First commands

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

# Materialize a browsable view from any frontmatter field
ntropy view add by-status --field status
```

The [Getting started](01M28Q327TJQ73DX24WBSXZHPF-getting-started.md)
section takes it from here.

## Why I built this

I live on the command line and in Neovim, and every note app I tried
wanted me back inside its own, usually graphical, UI to make sense of
files it nominally stored as plain text. I wanted the inverse: notes that
are just Markdown files, a CLI to manage them, and nothing stateful in
between. ntropy is the heavily opinionated result. Notes live flat in one
vault, a note's identity is a stable ULID rather than its title, and any
hierarchy you browse is a derived projection of the frontmatter instead
of the canonical storage. Switching vaults is cheap, one for work, one
for private, or a per-project vault pinned by a `.ntropy-vault` file in a
repo, so documenting a project is the same motion as any other note. It
scratched my itch; maybe it scratches yours.
