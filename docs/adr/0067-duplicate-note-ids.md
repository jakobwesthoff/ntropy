# 67. Duplicate note IDs

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0019](0019-scan-robustness-and-resource-tolerance.md), which
gains a reason to skip a file.

## Context

Copying a note file copies the ULID in its name. Every layer assumes one file
per ULID: link resolution took whichever of the two files the scan happened to
list first, and `reconcile` renamed a drifted note to its canonical name
through `rename(2)`, which replaces an existing destination, so a second file
holding that name lost its content.

## Decision

### The keeper

Among the files sharing a ULID, the keeper is the one with the shorter file
name, then the one first in byte order; the others are copies. The rule reads
only the names: copy tools append to a name (`plan-b`, `plan copy`, `plan 2`),
every machine that syncs the vault picks the same keeper, and a title edit,
which drifts a name from its title without changing it, cannot move the ULID
to another file. A copy saved under a shorter name than its original becomes
the keeper.

### Copies are skipped

The scan leaves each copy out of the notes and warns about it, naming the
keeper; `--strict` makes that fail. Searches, views, the site and link
resolution see the keeper alone, so a link to the ULID resolves to the keeper.
A full ULID used as a selector still matches the keeper and its copies, so
`edit <ULID>` and `delete <ULID>` treat it as ambiguous instead of picking one.
`new`, `today` and `write` report the copies their view refresh left out, as
`edit` does through the search that picks the note; other scan warnings stay
with the commands that list notes.

### No rename replaces a file

No rename of a note replaces an existing file. A destination that already
exists is refused, except a name the filesystem treats as the same entry:
on a case-insensitive filesystem, renaming `01arz…-note.md` to
`01ARZ…-note.md` only changes how the name is written. That case is
recognized by the same device and inode with a single link, so two hard
links of one file are refused, as `rename(2)` between them succeeds without
doing anything.

`reconcile` and the realignment after an edit or a `write` leave the note
under its old name and report the refused rename as a warning of its own,
`did not rename <from> to <to>`, which `--strict` makes fail. It is not a
scan warning, whose text says the file was skipped: the note is not skipped
and stays in searches and views. The note's content is saved either way.

The check and the rename are two calls; a second process creating that exact
name in between is not guarded against.
