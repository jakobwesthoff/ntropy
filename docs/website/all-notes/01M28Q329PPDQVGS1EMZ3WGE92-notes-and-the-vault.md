---
title: Notes and the vault
tags: [docs/notes]
site:
  index: true
  label: Notes and the vault
  order: 2
---
A note is a Markdown file, and a vault is the directory that holds your
notes. This section covers how notes start out from templates, which
vault a command works on, encrypted vaults, the note format, and the
Markdown ntropy understands.

## The notes are the database

ntropy keeps no index, cache, or hidden state. The Markdown files in your
vault are the single source of truth, and every command reads them fresh.
Everything else it shows you, such as readable dates, tag counts, or the
browsable view trees, is derived on demand. You can delete any of it and
have it rebuilt.

## The vault

A vault is an ordinary directory with a few well-known children:

```bash
~/notes/
├── all-notes/        # your notes, named <ulid>-<slug>.md: the source of truth
│   ├── 01j8z9k…-groceries.md
│   └── 01j8za2…-q3-planning.md
├── by-tag/           # a materialized view: symlinks grouped by the `tags` field
│   └── work/
│       └── 2026-06-24-q3-planning.md -> ../../all-notes/01j8za2…-q3-planning.md
├── by-status/        # another view, grouped by the `status` field
├── README.md         # seeded by `init`: what this directory is, how to get ntropy
└── .ntropy/          # config, templates, and themes
```

Only top-level `*.md` files in `all-notes/` are notes, with the extension
in lowercase. ntropy leaves subdirectories and all other files alone, a
`.MD` file included, so images and attachments can sit right next to
your notes without being taken for notes.

An encrypted vault stores `<ulid>.age` files in `all-notes/` instead, and
has no view directories; see
[Encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md).

## The vault in git

Because all of this is just files, the whole vault is yours to version:
`git init` in it and commit your notes like any other text. The view
directories are derived and do not belong in git. ntropy keeps them out
through the vault's `.gitignore`, as
[Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md)
describes.
