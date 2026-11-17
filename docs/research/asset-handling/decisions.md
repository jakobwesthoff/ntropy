# Decisions

The user's answers to [open-questions.md](open-questions.md), in the order
they were given. Each entry names the question, the answer and the reason
where the user gave one. A decision lands here only after the user gave it.

## Round 1, 2026-11-17 (asqr session `01M7MQ924SXDJWY7CBTQVB17MX`)

- **Question 1, folder naming: the folder follows the note file name.** A
  note's asset folder is named exactly like the note file without its
  extension, `<ULID>-<slug>`, and is renamed with the note. In an encrypted
  vault, where the note file is `<ULID>.age`, the folder is bare `<ULID>`.
  This is variant a in [options.md](options.md#readable-folder-names). It
  meets the user's principle that ntropy's structure stays readable and
  navigable without ntropy.
- **Question 1, when the folder follows a retitle: `reconcile` renames it.**
  `reconcile` renames the folder and rewrites the links into it in the same
  pass as it renames note files and rewrites note links, and prints the old
  and the new folder path. Between an edit and the next `reconcile` the
  folder keeps its old name.
- **Question 1, links in an encrypted vault: the link text stays, the
  folder is found by its ULID (enc-2).** Asset links keep the
  `assets/<ULID>-<slug>/` form in both storage modes, as note links keep
  `<ULID>-<slug>.md` (ADR 0028, ADR 0041). Renderers, exports and the
  language server find the folder by its ULID prefix in an encrypted vault,
  so `vault encrypt` and `vault decrypt` rename folders and rewrite no note
  body.
- **Question 2, location: `all-notes/assets/`.** Links read
  `assets/<ULID>-<slug>/<file>`, where the user's `primary` vault keeps its
  folders today.
- **Question 3, encrypt assets at all: not now.** The first asset release
  leaves asset encryption out. The user wants to work with the asset system
  first and decide on encryption afterwards, and does not use vault
  encryption at all today; whether ntropy keeps vault encryption is itself
  open. Questions 4 and 5, which depend on question 3, wait with it. The
  todos
  [01KZ9ZN4GKF9XCT143E1W5YSEJ](../../../todos/all-notes/01KZ9ZN4GKF9XCT143E1W5YSEJ-protect-non-note-assets-in-encrypted-vaults.md)
  (asset encryption) and
  [01M7MQZS5C380J2V8S7PJFSNA2](../../../todos/all-notes/01M7MQZS5C380J2V8S7PJFSNA2-decide-whether-ntropy-keeps-vault-encryption.md)
  (keeping vault encryption) carry it.

## Round 2, 2026-11-17 (asqr session `01M7MR3ESS2X74MN1AKHM6WZJF`)

- **Question 6, `delete`: the asset folder goes with the note.** When no
  other note references a file in it, `delete` removes the note's asset
  folder together with the note and says so in its confirmation.
- **Question 7, non-image files in a PDF: attached, with a size limit.**
  The limit is a vault setting, and a limit of `0` turns attaching off.
  The default and whether it applies per file or to the whole PDF follow
  in round 3.
- **Question 8, commands: no `new --attach` for now.** `asset add` and
  `asset list` are the commands; the user asked for examples of how they
  select a note and treat several matches, answered in round 3.
- **Question 9, links into another note's folder: not allowed.** Each note
  has its own assets. ntropy manages assets rather than leaving them to the
  user, so it does not nudge shared files to an unmanaged place either.
  Shared assets, if needed, come later as links or shared asset folders
  that ntropy manages. How a violating link is reported follows in round 3.
- **Question 10, the view symlink: wanted, the clash to be solved.** A
  note opened through a view should show its assets. The collision with a
  tag group named like the folder is to be removed, possibly by naming the
  asset folder differently; settled in round 3.

## Round 3, 2026-11-17 (asqr session `01M7MRJJPWRAFJ4T9P3SHX8TS2`)

- **Attachment limit: per file, 25 MiB by default.** Each linked non-image
  file up to the limit is attached to the PDF, a larger one is not and the
  render warns. The vault setting is `[render] attach_max`, and `0` turns
  attaching off. A per-file limit keeps the result independent of the order
  of the links.
- **The page shows that a file is attached.** `pdf.attach` stores the file
  in the PDF without drawing anything, so the link text carries a marker.
  The user asked to see styled variants before choosing one (round 4).
- **`asset add` and `asset list`: the draft fits, with `asset list`
  selecting its note exactly as `asset add`, `render` and `delete` do.**
- **Links into another note's asset folder are refused.** Renders and
  exports refuse such a link, not only the checks: each note has its own
  assets.
- **The asset folder is `all-notes/_assets/`.** A view's group directories
  are slugs, which never contain `_`, so every view directory can carry an
  `_assets` symlink without colliding with a tag group, and a note opened
  through a view shows its assets.
- **No conversion command.** The structures in use are made by hand and
  differ from vault to vault, so ntropy ships no command that converts
  them. The docs show, as an example, how an existing structure is moved
  into the asset layout by hand with the asset commands. This replaces the
  one-shot conversion command that options.md lists as settled.

## Round 4, 2026-11-17 (asqr session `01M7MS90BZKVVCTTZ7C2FF085Z`)

- **An attachment is found through the viewer's attachments panel.** The
  link in the PDF cannot open the attached file: typst's `pdf.attach`
  stores it at document level, a `/GoToE` link action targets only an
  embedded PDF and viewers support it unevenly, and the clickable form, a
  FileAttachment annotation, is something typst 0.15.1 does not draw. A
  todo holds the idea of adding that annotation by post-processing the PDF.
- **The marker is a badge with the size.** After the link text, a small
  tinted pill reads `attached · 1.2 MiB`, and a muted grey one reads
  `not attached · 240 MiB` for a file above the limit, in the theme's
  accent and muted colours.
- **`asset list` selects as described.** One note by ULID, file name, path
  or a query matching one; several matches open the picker on a terminal
  and are an error with `-n`.

With round 4 the design is decided except for asset encryption (questions 3
to 5), which waits until the asset layout has been used for a while.
