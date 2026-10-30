# 67. Duplicate note IDs

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0004](0004-note-identity-and-filename-strategy.md), under which
a note's ULID never changed after creation, and
[ADR 0019](0019-scan-robustness-and-resource-tolerance.md), which gains a
reason to skip a file.

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
with the commands that list notes. The warning tells to run `reconcile`.

### Copies get their own ULID

Only commands that already change the vault reassign: `reconcile`, and the
filename realignment after `edit`, `new`, `today` and `write`. Commands that
only read the vault warn. Each copy gets a new ULID that keeps the old one's
48-bit time part, so the note keeps its creation date, and is renamed to the
canonical name for it; its content does not change, as identity lives in the
filename alone (ADR 0004). The realignment after an edit handles every file
carrying the edited note's ULID, whether the edited note is the keeper or a
copy. `reconcile` prints `reassigned <from> -> <to>, its id stays with
<keeper>` on stdout and counts the copies in its summary; the realignment
after an edit or a `write` prints the same line on stderr after `note: `, so
`write`'s stdout stays the note's path.

The remaining 80 bits are derived, not drawn at random: they are the low bits
of a 128-bit FNV-1a hash of the old ULID, the copy's file name and a counter,
which counts up while the result names a ULID a file already claims. Every
machine that reassigns the same copy of a synced vault then computes the same
ULID. With a random part, two machines reconciling the same copy before a
sync tool such as Dropbox or Syncthing caught up would each rename it to a
different ULID, and the sync would keep both files.

In an encrypted vault, `reconcile` adopts a plaintext note dropped in by hand
by encrypting it under its ULID. A plaintext note whose ULID an encrypted
note already has, or that copies another plaintext note, first gets a ULID of
its own; the encrypted file of the shared ULID keeps it. The ULIDs already
taken are read from the file names, so this works on a locked vault.

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
