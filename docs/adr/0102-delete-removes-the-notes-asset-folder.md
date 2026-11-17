# 102. `delete` removes the note's asset folder

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0018](0018-cli-command-surface.md), whose `delete` entry says the
command removes a note and refreshes views.
Builds on [ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

`delete` removed one file and refreshed the views. The asset folder of the note
stayed, and a following `reconcile` found nothing wrong with it.

## Decision

`delete` removes the asset folder of the note with the note. The folder is
found by the ULID in its name, so a folder with a stale slug goes too. The
confirmation names the folder: `Delete <note> and its 3 assets (1.3 MiB)?`, and
a run that removed it prints `Deleted 3 assets (1.3 MiB)`. The count is of
every file at any depth except operating-system metadata (`.DS_Store`, `._*`,
`Thumbs.db`, `desktop.ini`), hidden files included, so it matches what goes.

When another file carries the note's ULID, the folder stays, whatever the flag
says: a copy, a file the scan skipped such as a merge-conflict file, or any
other file in `all-notes/` whose name leads with the ULID may be the note the
folder belongs to from now on. This is read from the file names in
`all-notes/`, as `reconcile` reads the ULIDs that are taken, and the warning
names those files. The folder goes with the last file that carries the ULID.

When another Markdown file refers to the folder, the folder stays. Such a file
is found by the text of every Markdown file in `all-notes/`, copies and skipped
files included, through the matcher that reads every `_assets/<name>` outside
code: the folder's ULID leading the name is what counts, however the reference
spells the rest of the name, and a raw HTML `<img src>` counts. A `warning:`
line names the folder, its file count and the files that refer to it, and the
run prints no asset line. `--remove-assets` removes the folder anyway. `--force` keeps its one meaning, skipping the
confirmation. A folder that stays is an orphan folder for `reconcile`
(ADR 0099), which reports it.

The note file is removed before the folder, so a crash leaves a folder that
`reconcile` reports and never a note whose links point nowhere.

A symlink in the folder is removed as a link and never followed. A symlink in
`_assets/` that the ULID leads, standing where the folder would be, is left
alone and a `warning:` line says so. An encrypted
vault has no managed asset folders and `delete` removes only the note there.

`fsutil::remove_dir_all`, which only the conversion to encrypted storage
called, is available to `delete` as well.
