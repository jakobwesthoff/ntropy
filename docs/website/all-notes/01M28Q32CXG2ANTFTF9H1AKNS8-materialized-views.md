---
title: Materialized views
tags: [docs/notes]
site:
  order: 5
---
ntropy stores notes flat, with no folders to file them into. Views are how you browse anyway. A view groups notes by one frontmatter field and materializes that grouping as a directory tree of symlinks, which you then navigate with whatever filesystem tools you already use. This page covers what a view is on disk, how to manage views, and how grouping values are normalized.

> [!NOTE]
> Views are unavailable in an [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md): a symlink tree would spell out your tag taxonomy in plaintext directory names inside the synced folder.

## What a view is on disk

A view is a top-level directory in the vault, with one subdirectory per grouping value and one symlink per note in that group. Every symlink points back into `all-notes/`:

```bash
by-status/
├── done/
│   └── 2026-06-24-q3-planning.md -> ../../all-notes/01J8ZA2…-q3-planning.md
├── in-progress/
└── todo/
```

Each leaf is named `<date>-<slug>.md`, where the date is the note's creation date and the slug comes from its title. When two notes in one group would get the same name, ntropy appends a short tail of each note's ULID to tell them apart. The links are relative, so a moved or copied vault keeps working.

Because the leaves are symlinks to the canonical files, there is still exactly one copy of every note; the view is another door into it. `cd` into it, `grep` it, point a file browser at it, open the links in any editor. ntropy refreshes views after every command that changes notes, and `ntropy reconcile` brings them back in sync after edits made outside ntropy. Both sync incrementally: only the links that changed are touched.

The view directories are derived data, so ntropy keeps them out of version control for you. It maintains an entry for every configured view in the vault's root `.gitignore`, marking its own lines so it never touches one you wrote.

## Managing views

Views are configured per vault, in `<vault>/.ntropy/config.toml`, and `ntropy view` manages that file:

```bash
ntropy view add by-status --field status   # group notes by their `status` field
ntropy view list                            # show configured views
ntropy view remove by-status                # drop the definition
```

`init` seeds a `by-tag` view on the `tags` field. Add one for whatever frontmatter field you navigate by: `status`, `project`, `author`, `area`, anything you put in your [notes](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md). A view's name must not be `all-notes`, `.ntropy`, or the name of another view.

`view remove` drops the definition and its `.gitignore` entry, but ntropy never deletes a directory. The now-stale view tree stays on disk and the command tells you so, so you can delete it yourself. Since its ignore entry is gone, git now sees that directory.

## How notes are grouped

- A list-valued field (like `tags`) fans a note out into every value it holds.
- A `/` inside a value (`area/roadmap`) nests into subdirectories.
- A note with no value for the field does not appear in the view.
- Grouping values are normalized the same way tags are, lowercased and slugified, so `In Progress` and `in-progress` land in the same directory. This is not configurable: a case-insensitive filesystem (APFS on macOS by default) cannot hold `Done/` and `done/` side by side.

## Views or search

Views are a convenience for when filesystem access is what you want, not the only way to slice your notes. Every field a view can group by, the [query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) can filter by too; `ntropy search status:done` needs no view at all. Make a view for a dimension you browse often, and use `search` for everything else.
