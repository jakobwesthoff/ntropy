---
title: Materialized views
tags: [docs/find]
site:
  order: 1
---
ntropy stores notes flat, with no folders to file them into. Views are
how you browse them as folders anyway. A view groups notes by one
frontmatter field and writes that grouping as a directory tree of
symlinks, which you navigate with whatever filesystem tools you already
use.

## Make a view

```bash
ntropy view add by-status --field status   # group notes by their `status` field
ntropy view list                            # show the configured views
ntropy view remove by-status                # drop the definition
```

`init` seeds a `by-tag` view on the `tags` field, unless the vault is
encrypted. Add one for whatever field you navigate by: `status`,
`project`, `author`, `area`, anything your notes carry. The views are
configured per vault, in `<vault>/.ntropy/config.toml`, which
`ntropy view` edits for you.

> [!NOTE]
> An encrypted vault has no views; see
> [Encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md).

## What a view is on disk

A view is a top-level directory in the vault, with one subdirectory per
grouping value and one symlink per note in that group. Every symlink
points back into `all-notes/`:

```bash
by-status/
├── done/
│   └── 2026-06-24-q3-planning.md -> ../../all-notes/01J8ZA2…-q3-planning.md
├── in-progress/
└── todo/
```

Each leaf is named `<date>-<slug>.md`: the note's creation date, then the
slug of its title. When two notes in one group would get the same name,
ntropy appends a short tail of each note's ULID to tell them apart. It
makes the tail longer where needed, so that no two names in a directory
differ only in letter case, which macOS's default file system would
treat as one file. The links are relative, so a moved or copied vault
keeps working.

Because the leaves are symlinks to the canonical files, there is still
exactly one copy of every note; the view is another door into it. `cd`
into it, `grep` it, point a file browser at it, open the links in any
editor. ntropy refreshes the views after every command that changes
notes. After edits made outside ntropy, `ntropy reconcile` brings them
back in sync. Both touch only the links that changed.

## How notes are grouped

- A list-valued field (like `tags`) fans a note out into every value it
  holds.
- A `/` inside a value (`area/roadmap`) nests into subdirectories.
- A note with no value for the field does not appear in the view.
- Grouping values are lowercased the way tags are, with punctuation
  kept. Each part of a value then becomes a directory name of letters,
  digits and dashes. So `In Progress` and `in-progress` land in the same
  directory, and `milestone/0.13` lands in `milestone/0-13`.
- Values that differ only in punctuation, such as `node.js` and
  `node-js`, share a directory with the notes of both. A value with
  nothing but punctuation, such as `...`, gets no directory.

None of this is configurable: a case-insensitive file system (APFS on
macOS by default) cannot hold `Done/` and `done/` side by side.

## Views and git

The view directories are derived data, so ntropy keeps them out of
version control for you. It keeps an entry for every configured view in
the vault's root `.gitignore`, and marks its own lines so it never
touches one you wrote.

ntropy never deletes a directory. `view remove` drops the definition and
its `.gitignore` entry, and tells you that the view's tree stays on disk
for you to delete. Since its ignore entry is gone, git now sees that
directory.

## View names

A view's name is its directory, and ntropy removes anything in that
directory that is not one of the view's links. So a name has to stay a
directory of its own inside the vault:

- It is made of lowercase letters, digits and single dashes, like
  `by-status`. A `/` nests it, as in `area/work`, and each part follows
  the same rule, at most 72 characters.
- It is not `all-notes`, `.ntropy`, `.gitignore` or `README.md`, and does
  not start with one of them followed by `/`.
- No part starts with a dot, so `.git` and `..` are out.
- It is not the name of another view.

`view add` refuses any other name and suggests the nearest valid one:

```bash
$ ntropy view add By_Tag --field tags
error: while adding the view: `By_Tag` is not a valid view name: the segment `By_Tag` may only hold lowercase letters, digits and single dashes, at most 72 characters (try `by-tag`)
```

A view in `config.toml` whose name breaks these rules, such as `By_Tag`
or `my.view` from an older ntropy, is left alone. Every command that
syncs views prints a warning for it, and the other views carry on.
`--strict` makes `reconcile` and `site` fail on it. Its old directory
stays on disk and stays ignored by git. To fix it, remove it, add it
again under a valid name, and delete the old directory:

```bash
ntropy view remove By_Tag
ntropy view add by-tag --field tags
rm -r By_Tag
```

`vault encrypt` refuses to start while such a view is configured, because
the old directory's link names would keep your note titles readable
inside the encrypted vault.

A view's directory must also be a real directory. If it, or a directory
above it, is a symlink, for example one that came along with a cloned
repository, ntropy skips the view with a warning. It does not follow the
link and empty whatever it points to. `view add` and `vault encrypt`
refuse in that case. Remove the symlink itself, not what it points to,
and the view syncs again.

## Views or search

Views are a convenience for when filesystem access is what you want, not
the only way to slice your notes. Every field a view can group by, a
query can filter by too: `ntropy search status:done` needs no view at
all. Make a view for a dimension you browse often, and use `search` for
everything else.
