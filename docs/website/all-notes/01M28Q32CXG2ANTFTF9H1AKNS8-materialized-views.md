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

Each leaf is named `<date>-<slug>.md`, where the date is the note's creation date and the slug comes from its title. When two notes in one group would get the same name, ntropy appends a short tail of each note's ULID to tell them apart, and makes it longer where needed so that no two names in a directory differ only in letter case, which macOS's default filesystem would treat as one file. The links are relative, so a moved or copied vault keeps working.

Because the leaves are symlinks to the canonical files, there is still exactly one copy of every note; the view is another door into it. `cd` into it, `grep` it, point a file browser at it, open the links in any editor. ntropy refreshes views after every command that changes notes, and `ntropy reconcile` brings them back in sync after edits made outside ntropy. Both sync incrementally: only the links that changed are touched.

The view directories are derived data, so ntropy keeps them out of version control for you. It maintains an entry for every configured view in the vault's root `.gitignore`, marking its own lines so it never touches one you wrote.

## Managing views

Views are configured per vault, in `<vault>/.ntropy/config.toml`, and `ntropy view` manages that file:

```bash
ntropy view add by-status --field status   # group notes by their `status` field
ntropy view list                            # show configured views
ntropy view remove by-status                # drop the definition
```

`init` seeds a `by-tag` view on the `tags` field. Add one for whatever frontmatter field you navigate by: `status`, `project`, `author`, `area`, anything you put in your [notes](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md).

`view remove` drops the definition and its `.gitignore` entry, but ntropy never deletes a directory. The now-stale view tree stays on disk and the command tells you so, so you can delete it yourself. Since its ignore entry is gone, git now sees that directory.

## View names

A view's name is its directory, and ntropy removes anything in that directory that is not one of the view's links. So a name has to stay a directory of its own inside the vault:

- It is made of lowercase letters, digits and single dashes, like `by-status`. A `/` nests it, as in `area/work`, and each part follows the same rule, at most 72 characters.
- It is not `all-notes`, `.ntropy`, `.gitignore` or `README.md`, and does not start with one of them followed by `/`.
- No part starts with a dot, so `.git` and `..` are out.
- It is not the name of another view.

`view add` refuses any other name and suggests the nearest valid one:

```bash
$ ntropy view add By_Tag --field tags
error: while adding the view: `By_Tag` is not a valid view name: the segment `By_Tag` may only hold lowercase letters, digits and single dashes, at most 72 characters (try `by-tag`)
```

If `config.toml` holds a view whose name breaks these rules, for example one added with an older ntropy that accepted `By_Tag` or `my.view`, ntropy leaves that view alone. Every command that syncs views prints a warning for it, the other views carry on, and `--strict` makes `reconcile` and `site` fail on it. Its old directory stays on disk and stays ignored by git. To fix it, remove it and add it again under a valid name, then delete the old directory:

```bash
ntropy view remove By_Tag
ntropy view add by-tag --field tags
rm -r By_Tag
```

`vault encrypt` refuses to start while such a view is configured, because the old directory's link names would keep your note titles readable inside the encrypted vault.

A view's directory must also be a real directory. If it, or a directory above it, is a symlink, for example one that came along with a cloned repository, ntropy skips the view with a warning instead of following the link and emptying whatever it points to. `view add` and `vault encrypt` refuse in that case. Remove the symlink itself, not what it points to, and the view syncs again.

## How notes are grouped

- A list-valued field (like `tags`) fans a note out into every value it holds.
- A `/` inside a value (`area/roadmap`) nests into subdirectories.
- A note with no value for the field does not appear in the view.
- Grouping values are normalized the same way tags are, lowercased with punctuation kept, and each part of a value becomes a directory name made of letters, digits and dashes, so `In Progress` and `in-progress` land in the same directory and `milestone/0.13` lands in `milestone/0-13`. Values that differ only in punctuation, such as `node.js` and `node-js`, share a directory with the notes of both. A value with nothing but punctuation, such as `...`, gets no directory. This is not configurable: a case-insensitive filesystem (APFS on macOS by default) cannot hold `Done/` and `done/` side by side.

## Views or search

Views are a convenience for when filesystem access is what you want, not the only way to slice your notes. Every field a view can group by, the [query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) can filter by too; `ntropy search status:done` needs no view at all. Make a view for a dimension you browse often, and use `search` for everything else.
