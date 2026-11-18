# 108. `asset list` fails under `--strict` on a broken link

Date: 2026-11-18

## Status

Accepted

Amends [ADR 0104](0104-asset-add-and-asset-list.md) and
[ADR 0106](0106-asset-list-prints-file-paths-or-the-folder.md), which leave the
exit code of `asset list` the same for every listing. Builds on
[ADR 0107](0107-asset-list-compares-a-links-folder-with-the-notes-folder.md).

## Context

`asset list` exited 0 whatever it listed, so a script could learn that a note's
link was broken only by reading the LINKED column. `search`, `show` and
`reconcile` fail under `--strict` on what they warn about.

## Decision

For every `broken` row, `asset list` prints a `warning:` line on stderr, in the
table form and with `--path`. A link into the note's folder whose file is
missing reads `<link> is linked but is not in the note's asset folder`. A link
that names another folder reads `<link> does not name the note's asset folder
_assets/<folder>/`. Both name the link as it is written in the note, and the
warnings come before the listing.

Under `--strict`, `asset list` exits 1 when it printed such a warning. Without
`--strict` it exits 0 as before, so a script that does not pass `--strict` is not
affected.

A file the note does not link (`no`) is no warning, and `reconcile` does not
count it either. `--folder` prints no warning and ignores broken links: it
prints the folder also when it does not exist yet, so that a script can create
it, and it shows no row.
