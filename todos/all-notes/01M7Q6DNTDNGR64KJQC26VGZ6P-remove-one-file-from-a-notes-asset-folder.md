---
title: "Remove one file from a note's asset folder"
kind: feature
component: asset
origin: implementation
---
# Remove one file from a note's asset folder

ntropy can add files to a note's asset folder and list them, but it has
no command that removes one. A script or agent that replaces a note's
attachments has to delete the file from `all-notes/_assets/<note>/`
itself. Found on 2026-11-18 while an agent skill that archives meeting
notes was moved to `ntropy asset add`: a replaced note's minutes can drop
a screenshot, and the leftover file then shows as `no` in `asset list`.

## Goal

A command such as `ntropy asset remove -n <note> <name…>` that deletes
the named files from the note's own folder, refuses a name outside it,
and says when the note still links a file it removed (that link then
reads `broken`).

## Open questions

- Should it refuse a file the note still links unless forced, or remove
  it and warn?
- Should it remove the folder when it ends up empty, as `delete` does
  with the note's folder?
- Encrypted vaults take no assets yet (ADR 0103); the command refuses
  there like `asset add` and `asset list`.
