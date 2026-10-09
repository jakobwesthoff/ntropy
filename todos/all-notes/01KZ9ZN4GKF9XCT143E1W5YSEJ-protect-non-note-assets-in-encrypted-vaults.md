---
title: "Protect non-note assets in encrypted vaults"
kind: feature
component: crypto
status: needs-discussion
impact: high
origin: discussion
---
# Protect non-note assets in encrypted vaults

Notes reference images, PDFs and other attachments as ordinary relative
Markdown links. Those files are not notes, and vault encryption
(`docs/design/encryption.md`) does not cover them. It encrypts
`all-notes/<ulid>.md` into `<ulid>.age` and says nothing about anything else
in the directory.

## Problem

The scanner treats non-note files as resources and leaves them alone. It
walks `all-notes/` at depth 1 only, keeps files whose extension matches the
vault's note extension, and silently ignores everything else. Subdirectories
are never descended into.

So in an encrypted vault every asset stays plaintext in the synced directory.
Its filename, size and contents all reach whoever stores or syncs the vault,
which is the party the threat model exists to defend against. An image named
`2026-q3-layoffs-org-chart.png` leaks as much as a note title would.

## Open questions

- Whether assets are encrypted at all, or stay plaintext with the leak stated
  explicitly in the threat model's "deliberately outside the model" list.
- How a note's link resolves if the asset becomes `<name>.age`. A body
  written as `![Chart](chart.png)` no longer points at a real file, and
  unlike note-to-note links there is no ULID to resolve by.
- How rendering gets the assets. The Typst engine runs with its working
  directory set to the note's own directory so relative assets resolve, which
  means it expects real files beside the note. Staging decrypted copies into
  the render staging directory would need the link targets rewritten to match.
- How an editor previews an encrypted image while editing a decrypted note in
  the runtime directory, where the asset is neither present nor readable.
- Whether subdirectories change anything, given the scanner does not descend
  into them but users do put asset folders there.

## Options

- Encrypt in place as `<name>.age`, decrypt into the render staging directory
  on demand, and rewrite link targets during staging. This closes the leak,
  but costs a resolution rule for non-note files and breaks plain-Markdown
  previewing entirely.
- Leave assets plaintext, document the leak, and let users keep sensitive
  attachments out of the vault. This is zero work, and the threat model gains
  an honest but fairly large hole.
- Give assets their own area with explicit handling, so the rules are stated
  rather than inherited from the resource-file convention.
