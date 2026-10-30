# 67. Duplicate note IDs

Date: 2026-10-30

## Status

Accepted

## Context

Copying a note file copies the ULID in its name. `reconcile` renamed a
drifted note to its canonical name through `rename(2)`, which replaces an
existing destination, so a second file holding that name lost its content.

## Decision

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
