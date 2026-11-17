# 104. `asset add` and `asset list`

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0018](0018-cli-command-surface.md), whose command list gains
`asset`. Builds on
[ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

A note's assets live in `all-notes/_assets/<stem>/` (ADR 0098). Putting a file
there by hand needs the folder's name, which changes with the note's title, and
leaves the link to the user.

## Decision

`ntropy asset add <note> <file>...` puts files into the note's asset folder and
prints a Markdown link for each. `ntropy asset list [--path] <note>` lists
what is in the folder.

Both work on exactly one note, selected as `render` and `delete` select theirs
(ADR 0025, ADR 0091): a ULID, a file name or path, or a query. Several matches
open the picker on a terminal and are an error under `-n`. Nothing is ever
added to several notes.

`asset add` never edits the note: it prints the links and the caller places
them. A link goes to stdout, `![stem](..)` for an image and `[name](..)` for any
other file, the target in angle brackets when the name holds anything but
letters, digits, `.`, `_` and `-`; one `added <path>` line per file goes to
stderr. `--move` moves the files instead of copying them, `--as <name>` stores
one file under another name, and `--force` replaces a file of that name, which
is otherwise refused. Every check runs before the first file is touched. An
I/O failure during the transfer leaves the files already placed: the command
prints the `added` line and the link of each of them, then the error, which names
the file and the cause. A move whose copy succeeded and whose source could not be
removed prints the file as stored and says it is now in both places. A view that
cannot be refreshed after the files are in place is a warning that counts for
`--strict`, printed after the links. A note of the vault (a `.md` or `.age`
file directly in `all-notes/`, however its path is spelled or reached) is
refused as a source. A move out of another note's asset folder warns, naming the
folder, since the links that note holds to the file break; the move still
happens, and the warning counts for `--strict`. A file that is
already the file it would be stored as, reached by the same path, through a
symlinked directory such as a view's `_assets` link, by a hard link or as a
symlink to it, is refused, since the move or the forced copy would remove the
only copy. Names are compared without regard to case, since a vault moves
between case-sensitive and case-insensitive filesystems: two files that would be
stored under names that differ only in case, and a file whose name differs only
in case from one already in the folder, are refused, also with `--force`, which
replaces exactly the file of that name. The comparison folds each character to
upper case and back to lower case, so `ß` and `ss` or `ς` and `σ` meet.
Spellings that differ only in canonical equivalence (an accent as one character
or as two) are caught at transfer time: the copy stops instead of replacing a
file this run has just stored. A name whose
link the Markdown parser of ADR 0097 would not read back as the file is refused
with the hint to use `--as`.

The folder is found by the ULID of the note, under whatever name it has. A note
whose folder has not been renamed since a retitle gets the file in that folder
and a link that names it, and a note with several folders is refused. A symlinked `_assets` directory, or
a symlink where the note's folder belongs, is refused too by both commands: the
entry is one ntropy leaves alone (ADR 0098), and a file added or listed through
it would belong to another note or to a place outside the vault.

`asset list` prints the files of the folder with their size, hidden files
included and operating-system metadata left out as `delete` leaves it out, and
whether the note links each, a file that raw HTML in the note names counting as
linked, and a file a Markdown link of the note names in its own folder and the
folder lacks as `broken`: only Markdown links make a file missing, as `reconcile`
reports it. `--path` prints only the folder, also when it does not exist yet. The
unlinked files of a folder show in `asset list` only (ADR 0099).

In an encrypted vault both commands fail with an error saying that asset
encryption is not supported yet (ADR 0103).

There is no `new --attach`, and there is no command that converts an existing
structure into this layout: how one is moved by hand with these commands is
shown in the user documentation.
