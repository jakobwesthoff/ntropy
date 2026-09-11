---
title: Notes and the vault
tags: [docs/notes]
site:
  index: true
  label: Notes and the vault
  order: 2
---
This section covers what a note is, where it lives, and what ntropy keeps
around it: the vault directory, the note file format, the Markdown it
understands, templates, materialized views, encryption, and configuration.

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

Only top-level `*.md` files in `all-notes/` are notes. ntropy leaves
subdirectories and non-`.md` files alone, so images and attachments can sit
right next to your notes without being adopted as notes.

A vault can also store its notes [encrypted at
rest](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md), in which case
`all-notes/` holds `<ulid>.age` files instead and there are no view
directories.

Because all of this is just files, the whole vault is yours to version: `git
init` in it and commit your notes like any other text. The derived `by-*/`
view directories do not belong in git, and ntropy keeps them out for you. It
maintains a root `.gitignore` whose managed entries always match your
configured views, adding one when you add a view and pruning it when you
remove one. Each managed entry carries a marker comment above it, so ntropy
only ever prunes its own entries. Your own lines in that file are never
touched.

ntropy never deletes a directory. When a view is removed its directory is
left behind, and, no longer ignored, it shows up in `git status`. The command
tells you so you can delete the stale tree yourself.

## In this section

- [Note format](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md): the frontmatter
  fields, what the filename means, and how notes link to each other.
- [Markdown flavor](01M28Q32B13V8F4Z4CHE258TC5-markdown-flavor.md): the
  GitHub-flavored Markdown ntropy understands, and what it leaves alone.
- [Templates and daily
  notes](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md): how
  `new` stamps out a note, the placeholders, and `today`.
- [Finding the vault](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md): how
  a command decides which vault it works on.
- [Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md):
  symlink trees grouped by a frontmatter field.
- [Encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md): notes
  encrypted at rest with age.
- [Configuration](01M28Q32E6BHWB0H27MM57KY90-configuration.md): the
  `.ntropy/config.toml` file.
