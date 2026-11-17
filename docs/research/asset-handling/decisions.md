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
