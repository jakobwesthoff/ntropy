---
title: "asset list reports a link into a missing folder as linked"
kind: bug
component: asset
origin: review
---
# asset list reports a link into a missing folder as linked

`ntropy asset list` shows `yes` under LINKED for a link whose folder
name starts with the note's ULID but is not the note's asset folder,
even when that folder does not exist. A script or agent that checks a
note's assets with `asset list` then accepts dead links. Found on
2026-11-18 in an adversarial review of an agent skill that archives
notes with a transcript and screenshots as assets.

## Problem

`src/asset/files.rs`, `list` (around line 682), counts a link as the
note's own when `owner(&folder) == Some(note.id)`, that is by the ULID
that leads the folder name in the link. The rest of the folder name is
never compared with the real folder, so only the file name decides
between `yes`, `no` and `broken`.

Reproduced on 2026-11-18 with ntropy 2.14.0 in a scratch vault: a note
whose files sit in `_assets/<ULID>-release-check-mit-anna/` and whose
links were rewritten to `_assets/<ULID>-wrong-slug/transkript.txt` and
`_assets/<ULID>-wrong-slug/screenshot-103000-helm-values.png`.
`asset list -n <ULID>` lists both files as `yes` with exit code 0, and
`ntropy --strict search -n <ULID>` prints no warning. `ntropy reconcile
-n` then relinks both links to the real folder.

The same state arises in normal use: after a retitle the folder keeps
its old name until `reconcile` runs, so links written with the new name
point at a folder that is not there yet; and links written with a
basename that `write` then changes point at a folder that never comes
into being.

## Impact

`asset list` cannot be used to prove that a note's asset links resolve.
Callers have to test every link path themselves or run `reconcile`,
which works on the whole vault.

## Suggested fix

In `list`, compare the link's folder with the note's real asset folder.
A link whose ULID matches but whose folder name differs counts as
`broken` (or as a separate state that says `reconcile` will relink it),
so the file it names shows as `no` until the link is fixed.

## Open questions

- Should a link that `reconcile` would relink get its own state, so the
  listing can say which command fixes it?
- Should `asset list` exit non-zero under `--strict` when a row is
  `broken` or `no`? Today it exits 0 for every state, so a script has to
  parse the LINKED column.

## Relations

- Relates to: [A link into a folder named like a note is read as a link to that note](01M7MNSS8YXMWC461EWYT9HFFD-a-link-into-a-folder-named-like-a-note-is-read-as-a-link-to-that-note.md), both read a link's target by its leading ULID alone
