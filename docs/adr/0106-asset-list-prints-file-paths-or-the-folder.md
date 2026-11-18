# 106. `asset list` prints file paths or the folder

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0104](0104-asset-add-and-asset-list.md), where `asset list --path`
prints the folder: `--path` now prints the asset files and the new `--folder`
prints the folder.

Amended 2026-11-18 by
[ADR 0107](0107-asset-list-compares-a-links-folder-with-the-notes-folder.md).

Amended 2026-11-18 by
[ADR 0108](0108-asset-list-fails-under-strict-on-a-broken-link.md).

## Context

A script working on a note's assets needs the path of each file and the path of
the folder.

## Decision

`ntropy asset list --path <note>` prints the absolute path of every file the
plain listing shows with a size, one per line, in the listing's order. Hidden
files and files in subfolders are included, operating-system metadata is left
out as in the listing. A link the listing marks `broken` is not printed: the
folder holds no file of that exact name. A symlink in the folder is a row of the
listing and is printed as its own path, whatever it points to. A note without
asset files prints nothing and succeeds.

`ntropy asset list --folder <note>` prints the folder's absolute path, also when
it does not exist yet.

`--path` and `--folder` cannot be combined.

The note is selected as for the plain listing. Both forms count as `-p` for the
picker: a cancelled picker fails, so it is told apart from a note without asset
files, which prints nothing and succeeds. The refusals of ADR 0104 (a symlinked
`_assets` or note folder, an encrypted vault, several folders) apply to all
three forms.

`--path` prints nothing and fails when a path cannot be printed as it is: a name
that holds control or bidi characters (ADR 0071), or a name that is not valid
UTF-8, which the listing decodes lossily and which would print as a path that
does not exist. The error names the name.
