# 103. Encrypted vaults take no assets yet

Date: 2026-11-17

## Status

Accepted

Builds on [ADR 0041](0041-opt-in-vault-encryption-with-age.md) and
[ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

Vault encryption covers note files only. The `Cipher` seam reads and writes
text, so a binary file has no path through it, and the files in an asset folder
stay plaintext. A note in an encrypted vault is `<ULID>.age`, with no slug
(ADR 0041), and the asset folder of ADR 0098 is named with one.

## Decision

An encrypted vault takes no assets in this release.

- `vault encrypt` refuses to start while `all-notes/_assets/` holds a file or a
  symlink other than operating-system metadata (`.DS_Store`, `._*`,
  `Thumbs.db`, `desktop.ini`). The error names the first such file and says
  that asset encryption is not supported yet. Empty folders pass. Another
  hidden file, such as `.env`, blocks, since it would stay plaintext beside the
  encrypted notes. The check runs before the vault's key pair is written, as
  the check for copied notes does, so a refused vault is still a plaintext
  vault.
- `vault encrypt` refuses to start while a view tree holds a regular file other
  than operating-system metadata, naming it. The conversion removes the view
  trees, and a file in one, such as a file an editor saved through the view or
  a real `_assets` directory that kept its place (ADR 0100), would be deleted
  with it. The leaf links and the `_assets` links of the views are symlinks and
  do not count.
- `reconcile` and `delete` do no asset work in an encrypted vault.

The design for a later release keeps a bare `<ULID>` folder name in an
encrypted vault, where the note file is `<ULID>.age`. Links keep the plaintext
form `_assets/<ULID>-<slug>/<file>` in both storage modes, as note links keep
`<ULID>-<slug>.md`, and renders, exports and the language server resolve the
folder by its ULID prefix. `vault encrypt` and `vault decrypt` then rename
folders and rewrite no note body. None of this is built.
