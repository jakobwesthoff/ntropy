# 99. `reconcile` checks asset links and the assets directory

Date: 2026-11-17

## Status

Accepted

Builds on [ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

A note's assets live in a folder of their own (ADR 0098), and each note has
only its own assets. Files outside `all-notes/_assets/`, in a vault-root
`assets/<topic>/` or beside the notes, render and export as before.

## Decision

After it has renamed folders and rewritten links, `reconcile` warns about:

- a link to a file missing from the note's own folder;
- a link into another note's folder, or into `_assets/` outside the folder of
  any note, and a reference to such a folder in raw HTML, such as an `<img src>`,
  which the parser reads as HTML and not as a link;
- a link to a file outside `all-notes/_assets/`, which ntropy does not manage;
- a folder in `_assets/` that no valid note owns, such as the folder of a note
  that is gone;
- a file directly in `_assets/`, outside every note's folder, which is worded as
  that and not as an orphan folder;
- a symlinked `_assets/` or a symlinked entry in it, which ntropy leaves alone.

Each warning counts in the summary and fails `--strict`. A folder whose name is
stale is none of them: its note owns it, and `reconcile` renames it first, so a
run that renamed a folder reports nothing about it. Nothing is deleted: an
orphan folder is reported and stays.

Links are found by the Markdown parser pass of ADR 0097, so a link inside code
is not checked, and a raw HTML reference is checked only for the folders of
other notes, from the occurrences of `_assets/<name>` outside code. A link to a note, a URL, a fragment of the same
note and a link that climbs out of the vault are not reported. Each file is
reported once per note. Entries whose name starts with `.` are ignored, so a
`.DS_Store` is not an orphan.

Old structures, such as `all-notes/assets/<ULID>-<slug>/` or
`all-notes/images/`, get no report of their own. Their links are links to files
outside `all-notes/_assets/`.

An encrypted vault is not checked.
