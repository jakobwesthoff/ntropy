---
title: Note format
tags: [docs/notes]
site:
  order: 4
---
A note is a plain Markdown file with a YAML frontmatter block. This page
describes the frontmatter fields, what the filename carries, and how notes
link to each other.

## Frontmatter

The schema is permissive on purpose: any fields you write are kept, and every
one of them becomes filterable just by existing.

```markdown
---
title: Q3 Planning
tags: [work, planning, area/roadmap]
status: in progress
due: 2026-07-01
---
# Q3 Planning

Whatever you want below the frontmatter.
```

Two fields carry special meaning. The rest are yours.

- `title` is required and is the canonical, human title: full case,
  punctuation, and Unicode. The filename slug is derived from it, so the
  title is the truth and the slug is a readable echo. A note with no `title`,
  or with a blank one, is malformed: ntropy skips it with a warning, or fails
  under `--strict`. A number or `true`/`false` is read as its text, so
  `title: 2026` is the title "2026". YAML does not keep how a number was
  written, so `1.50` reads as `1.5` and `0x1F` as `31`; quote a title whose
  spelling matters. A list or a mapping as the title is malformed too, and the
  warning says which it found.
- `tags` is a flat list of strings. A forward slash denotes hierarchy by
  convention: `area/roadmap` is one tag with two levels, which both queries
  and views understand. ntropy normalizes each tag when it reads the note:
  segments are lowercased and whitespace becomes a dash, so `Rust` and
  `rust` are the same tag, and exact duplicates collapse into one.
  Punctuation stays as written, so `milestone/0.13`, `c++` and `node.js`
  are listed and matched exactly like that. A lone string instead of a list
  is accepted as a single tag. Numbers and booleans are read as their text,
  as in a title (`tags: [2026, rust]` holds both), and quoting a tag keeps
  its spelling. A list or a mapping inside `tags` makes the note malformed,
  and the warning names the entry by its position.
- Everything else (`status`, `due`, `author`, anything you like) is a free
  field. Filter on it, build a view from it, or just keep it for yourself.

You never write the date or id by hand. A note's id is the ULID in its
filename, and its creation date is derived from that ULID. The modification
date comes from the file's mtime. Neither is stored in the frontmatter, so
there is nothing to keep in sync.

When ntropy rewrites a note (during `reconcile`, say), it preserves the
frontmatter bytes as they are, so fields it does not recognize survive
untouched.

## The filename

Each note in `all-notes/` is named `<ulid>-<slug>.md`. The ULID is 26
characters, generated when the note is created, and is the note's identity.
The slug is a lowercased, ASCII form of the title, capped at 72 characters.
Every ASCII character other than a letter, a digit or a dash, a dot for
example, becomes a dash, so `Node.js notes` is filed as `node-js-notes`. An
apostrophe is dropped, so `Don't Panic` is `dont-panic`. A title that normalizes to nothing gets the slug
`untitled`. Because the ULID leads and is millisecond-precise, a plain lexical
sort of `all-notes/` is chronological.

When you change a title, the slug drifts. `ntropy reconcile` renames the
file to match again, and ntropy does the same for a single note when you
close the editor it opened for you. In an encrypted vault the file is
named `<ulid>.age` with no slug at all, so there is nothing to drift.

Copying a note file by hand copies its ULID, and two files then claim one
identity. ntropy keeps the one with the shorter file name as the note. It
skips the other with a warning that names the note it kept, so searches,
views and links see one note per ULID. Passing that ULID to `search` or
`delete` still matches both files. `ntropy reconcile`, or editing or
writing either file through ntropy, gives the copy a ULID of its own with
the same creation time and says so. Links that used the shared ULID keep
pointing at the note that kept it.

## Linking between notes

Notes link to each other with ordinary Markdown links. Nothing custom:

```markdown
See [the Q3 plan](01j8za2…-q3-planning.md) for the numbers.
```

The target is the note's filename. The leading ULID is the note's real
identity, so the link keeps resolving even after the target's title and
slug change: ntropy reads the first 26 characters of the target as a ULID
and looks the note up by that. `ntropy reconcile` rewrites the slug part
of existing links, so the readable part stays accurate and the link stays
clickable in a plain viewer. They are ordinary Markdown links, so GitHub,
your editor's preview, and any other Markdown tool follow them.

In a PDF, a web page and an exported site, a note link shows the target's
current title instead of the text you wrote.

A link to a heading of another note adds the heading's id after a `#`:

```markdown
See [Inside tmux](01j8za2…-notifications.md#inside-tmux).
```

`ntropy reconcile` refreshes the file name and keeps the `#inside-tmux`.
A link with an anchor shows the text you wrote, not the note's title.
`ntropy site` warns about an anchor that matches no heading of the target.
Where such a link leads in a PDF and on a web page is described in
[Rendering to PDF and HTML](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md).

A link counts as a note link only when its target starts with a valid ULID
and ends in `.md`, optionally followed by `#` and a heading id. Anything
else, such as external URLs, in-page anchors, or a ULID that matches no
note, is left alone. Image links (`![..](..)`) and links inside code are
never treated as note links, and `reconcile` does not rewrite them. Code
means inline code spans, which may continue over a line break, fenced
code blocks and indented code blocks.

Links keep the `<ulid>-<slug>.md` form in an encrypted vault too, even
though the files there are named `<ulid>.age`. Resolution goes through the
ULID, not the filesystem, so encrypting or decrypting a vault rewrites no
note bodies.

You can type these links by hand, but you do not have to: the
[Language server](01M28Q32MG1WQ0BTSBGCER8A2R-language-server.md) completes
them in your editor.
