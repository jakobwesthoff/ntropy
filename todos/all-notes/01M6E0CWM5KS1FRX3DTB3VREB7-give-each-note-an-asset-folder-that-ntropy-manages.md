---
title: "Give each note an asset folder that ntropy manages"
kind: feature
component: vault
horizon: next
origin: incident
---
# Give each note an asset folder that ntropy manages

ntropy has no concept of a note's assets: screenshots, transcripts and other
attachments live wherever the user or an agent puts them, by a convention
ntropy knows nothing about, so nothing keeps them in step with their note.
On 2026-11-02 this broke in the user's `primary` vault, when a `reconcile`
renamed notes and the asset folders named after two of them kept their old
names. The design that fixes it was researched and decided with the user on
2026-11-17; this todo carries its implementation, the first asset release.

## The decided design

Everything below is in `docs/research/asset-handling/decisions.md` (rounds
1 to 5), with the research behind it in the same directory.

- Each note's assets live in `all-notes/_assets/<note file name without
  extension>/`, so the folder stays readable without ntropy. In an
  encrypted vault, where the note file is `<ULID>.age`, the folder is bare
  `<ULID>`; links keep their plaintext form and resolve by the ULID prefix,
  so `vault encrypt` and `decrypt` rename folders and rewrite no note body.
- `reconcile` renames the folder together with the note, rewrites the
  links into it in the same pass and prints the old and the new path.
- Every view directory carries an `_assets` symlink, so a note opened
  through a view shows its assets; `_` never occurs in a group directory's
  slug, so the name cannot collide with a tag group.
- Each note has only its own assets: a link into another note's folder is
  refused by renders and exports, not only reported.
- `delete` removes the note's folder with the note and says so.
- `ntropy asset add <note> <file…>` copies (or `--move`s) files into the
  folder and prints the Markdown link for each, never editing the note;
  `ntropy asset list [--path] <note>` lists a note's files with size and
  whether the note links them. Both select their note as `render` and
  `delete` do: ULID, file name, path, or a query matching one note; several
  matches open the picker on a terminal and are an error with `-n`. No
  `new --attach`.
- In a PDF, a linked non-image file in the note's folder is attached with
  typst's `pdf.attach` up to a per-file limit, `[render] attach_max`
  (default 25 MiB, `0` turns attaching off), and the link carries a badge
  (`attached · 1.2 MiB`, or `not attached · 240 MiB` above the limit).
- Links to files outside every `_assets/` folder (beside the notes, or a
  vault-root `assets/<topic>/`) keep working; `reconcile` warns about them,
  and `--strict` fails on that. Old structures get no report of their own.
- `reconcile` reports an `_assets/` folder whose note is gone and deletes
  nothing; unlinked files show in `asset list` only. `--strict` fails on a
  missing file in the own folder, a link into another note's folder and an
  orphan folder, not on a stale folder name before the next `reconcile`.
- In an encrypted vault the asset commands refuse with an error until
  asset encryption exists.
- No conversion command: existing hand-made structures differ per vault, so
  the docs show by example how one is moved into the layout by hand with the
  asset commands.

## Not in the first asset release

Asset encryption (todo 01KZ9ZN4GKF9XCT143E1W5YSEJ), shared assets between
notes (later, as links or shared folders ntropy manages), and a clickable
attachment annotation in the PDF (todo 01M7MSGRW32068M8DK5E10WMGX).

## Before implementing

ntropy's own docs vault (`docs/website`) keeps 15 screenshots in a
vault-root `assets/` folder, which the warning above covers. None is shared
(checked 2026-11-17): `assets/document-theme/step-1..7.png` belong to
"Writing a document theme" (`01M6CAJG6C3WAZ2MD6W3EGBMKY`),
`assets/site-theme/01..06` to "Writing a site theme"
(`01M6CBSVXZXEW713FS476VB76F`), `07` and `08` to "Making a front page"
(`01M6CBSVZBM74V17MGTHGWYECR`). The asset release moves them into those
notes' `_assets/` folders with `asset add --move` and serves as the worked
example of a manual conversion. The theme-test note "Raw HTML and remote
images" (`docs/website/theme-test/`) links `step-1.png` by its published
URL, which changes with the move, so that link moves too.

Write the ADRs for the layout and the commands; the research's
`options.md` lists the technical points the readable naming opens (finding
asset links with a pulldown-cmark pass and the renderer's options, the
`realign` behaviour between an edit and the next `reconcile`, resolving by
the ULID prefix in encrypted vaults). Its "settled" one-shot conversion
command is replaced by the docs-only decision above.

## Relations

- Relates to: [Protect non-note assets in encrypted vaults](01KZ9ZN4GKF9XCT143E1W5YSEJ-protect-non-note-assets-in-encrypted-vaults.md), encryption of the assets this todo gives a defined place
