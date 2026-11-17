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
files at any depth and leaves out hidden entries.

When another note links a file in the folder, the folder stays. Another note is
one with a link whose folder the deleted note's ULID leads, however the link
spells the folder name. A `warning:` line names the folder, its file count and
the notes that link it, and the run prints no asset line. `--remove-assets`
removes the folder anyway. `--force` keeps its one meaning, skipping the
confirmation. A folder that stays is an orphan folder for `reconcile`
(ADR 0099), which reports it.

The note file is removed before the folder, so a crash leaves a folder that
`reconcile` reports and never a note whose links point nowhere.

A symlink in the folder is removed as a link and never followed. An encrypted
vault has no managed asset folders and `delete` removes only the note there.

`fsutil::remove_dir_all`, which only the conversion to encrypted storage
called, is available to `delete` as well.
