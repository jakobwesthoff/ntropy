# 107. `asset list` compares a link's folder with the note's folder

Date: 2026-11-18

## Status

Accepted

Amends [ADR 0104](0104-asset-add-and-asset-list.md) and
[ADR 0106](0106-asset-list-prints-file-paths-or-the-folder.md), which decide
what `asset list` shows for a link. Builds on
[ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

`asset list` took a Markdown link for a link into the note's folder when the
folder name in the link carries the note's ULID, and compared nothing else of
the name. A link into `_assets/<ULID>-wrong-slug/` marked a file of the note's
real folder as linked, also when no folder of that name exists.

## Decision

A Markdown link links a file of the note's folder when its target names the
folder the note has: the one it owns under its present name, which is the old
name between a retitle and the next `reconcile` (ADR 0104), or the note's file
name when it owns none. The folder names are compared exactly, letter case
included, and the target as the Markdown parser reads it, without percent-decoding. A link that names
a directory of the folder is satisfied, as `reconcile` treats it.

A link that carries the note's ULID and names another folder is a link to a file
the folder lacks. It is listed as `broken`, with no size, and its FILE column
holds the link's target as the Markdown parser reads it, without its fragment, so that it cannot be taken for
a file of the folder. The file of the folder with the same name is `no` unless
another Markdown link or raw HTML names it. Links to one target are one row.
`--path` prints no row without a size (ADR 0106).

Raw HTML references already compare the folder and stay as they are: a reference
into another folder links nothing and never makes a row `broken`.

`reconcile` treats the same link in one of three ways:

- It names the note's file name and the folder has an older name: `reconcile`
  renames the folder and the link resolves.
- It names another folder and the folder named like the note exists, with no
  entry of the link's name in `_assets/`: `reconcile` rewrites the link
  (ADR 0098).
- The note has no folder: `reconcile` reports a missing file.

On a case-insensitive filesystem a link whose folder or file name differs from
the real one only in letter case resolves there, and `reconcile` finds the file,
while `asset list` shows `broken`.
