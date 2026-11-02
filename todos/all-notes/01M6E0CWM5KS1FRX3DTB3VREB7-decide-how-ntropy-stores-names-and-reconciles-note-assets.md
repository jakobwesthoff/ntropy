---
title: "Decide how ntropy stores, names and reconciles note assets"
kind: investigation
component: vault
status: needs-discussion
origin: incident
tags: [encryption]
---
# Decide how ntropy stores, names and reconciles note assets

ntropy has no concept of a note's assets. Screenshots, transcripts and
other attachments live wherever the user or an agent puts them, by a
convention ntropy knows nothing about, so nothing keeps them in step
with their note. On 2026-11-02 this broke in the `primary` vault: a
`reconcile` run renamed 20 notes under the current slug rules, and the
asset folders named after two of them kept their old names. The goal is
one defined way to create, find, rename and remove a note's assets,
with encryption as a later step on top of it.

## Context

The convention in use puts a note's files in
`all-notes/assets/<note basename>/` and links them as
`assets/<note basename>/<file>`. It comes from the meeting-minutes
workflow, not from ntropy. What went wrong on 2026-11-02:

- `01M5WAPA7WSW68JVT9KN9N9XX6-patch-day-nodejs-…` became
  `…-patch-day-node-js-…` through `reconcile`. Its asset folder kept
  `nodejs`. The links still resolved because they named the old folder,
  so nothing warned about it.
- `01JJP1G2GQ6D0HAZZN1T0GA80K-…-dass-verbessert` had an asset folder
  ending in `-dass-verbessert-werd`, apparently from an earlier, longer
  slug. It went unnoticed until the same cleanup.
- Absolute paths to assets copied outside the vault (meeting notes in
  Muesli) broke with the folder rename and had to be fixed by hand.

Both folders were renamed and their links fixed by hand. `reconcile`
updates link targets between notes but never touches asset folders or
links to them. The scanner reads `all-notes/` at depth 1 only, so it
does not see asset folders at all.

## Options

- **Folder named by ULID alone** (`assets/<ULID>/`). Titles and slug
  rules can change without touching the folder, and paths copied
  outside the vault stay valid. The folder name no longer says what the
  note is about.
- **Folder named by note basename, kept current by `reconcile`.**
  Readable, but every retitle or slug rule change renames the folder
  and rewrites the asset links in the note. Paths outside the vault
  still break.
- **Keep it a user convention** and only add a check (in `reconcile
  --strict` or a lint command) that reports asset links pointing at
  missing files and asset folders without a matching note.

## Open questions

- Commands: does ntropy create the folder (`new --assets`, an
  `asset add <ULID> <file>` command), print its path for scripts, and
  delete it with the note?
- How links to assets are written so that `reconcile`, the renderers
  and the site export can resolve them without guessing, and whether
  they need the ULID in them the way note links do.
- What happens to assets shared by several notes.
- Encryption, as a follow-up once the layout is settled: a defined
  asset area per note would answer several questions the related todo
  leaves open, such as how a link resolves when the file becomes
  `<name>.age`.

## Relations

- Relates to: [Protect non-note assets in encrypted vaults](01KZ9ZN4GKF9XCT143E1W5YSEJ-protect-non-note-assets-in-encrypted-vaults.md), encryption of the assets this todo gives a defined place
