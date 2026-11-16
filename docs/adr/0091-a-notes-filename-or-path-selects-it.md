# 91. A note's filename or path selects it

Date: 2026-11-16

## Status

Accepted

Amends [ADR 0025](0025-plain-output-format-ordering-and-edit-ambiguity.md),
whose selector was a full ULID or a query, and
[ADR 0043](0043-write-command-for-note-content.md), whose `write` named its
target by ULID, filename or a path into `all-notes/`.

## Context

`search`, `delete`, `render` and `show` took a full ULID or a query, while
`write` also took a note's filename or path. A query that selects one note
today can match a second one later, while a path, as `search -p` prints it,
names one file.

## Decision

`search` (aliases `list` and `edit`), `delete`, `render` and `show` take a
note's filename or path in place of a full ULID or a query. `write` resolves
its target by the same lookup.

- A selector is a path when it is the whole selector, as a full ULID is, ends
  in `.md` or `.age` and names an existing file. Deciding by an existing file,
  combined with the suffix, keeps ordinary queries from being taken for paths.
- A bare filename is looked up in `all-notes/`. Any other path is taken as
  given, relative to the working directory or absolute.
- The file must be directly in `all-notes/` and stored the way the vault stores
  its notes: `.md` in a plaintext vault, `.age` in an encrypted one. A symlink
  to such a file selects that note, so a view entry selects the note it points
  to. A symlink whose target is no such file is no note.
- An existing file that is no note of the vault is an error that says so,
  instead of a query that matches nothing. That covers a file elsewhere in the
  vault, such as a README or a template, a bare name found in the working
  directory but not in `all-notes/`, and a file stored the other way.
- The name of a note file, `<ulid>-<slug>.md` or `<ulid>.age`, whose file does
  not exist is an error, `no note <name> in the vault's all-notes/`. As a query
  it would match the notes that link to the old name, so a script with a stale
  name fails instead of acting on the wrong note.
- Any other `.md` or `.age` argument that names no file is a query, as is a
  path among several arguments.
- A path selects exactly the file it names. The ULID of a note and a copy of it
  selects both (ADR 0067); the path of the copy selects the copy.
- A file directly in `all-notes/` that the scan skips selects no note, and the
  scan's warning says why.
- Resolving a name reads no note, so a locked vault tells a file that is no
  note from one that is; reading the selected note needs the key.

The rule is stated in the `--help` text of the commands and in the design
docs. The library lookup is shared: `ops::locate` resolves a name for `write`
and for the selector, and `ops::Selector` carries the typed arguments to it,
as composing them into a query turns an argument that holds spaces into a
pattern.
