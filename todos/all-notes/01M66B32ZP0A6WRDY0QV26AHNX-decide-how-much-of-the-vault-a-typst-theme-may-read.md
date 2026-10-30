---
title: "Decide how much of the vault a Typst theme may read"
kind: investigation
component: render
status: needs-discussion
origin: review
tags: [security]
---
# Decide how much of the vault a Typst theme may read

Found on 2026-10-30 while ADR 0073 was planned. A Typst theme
(`.ntropy/themes/typst/*.typ`, ADR 0045) is code that runs with read access to
the whole vault: `render --to pdf` passes the vault root as `--root`
(`src/render/typst/engine.rs`), so a theme can `read()` any file in it and put
its text into the PDF. That includes `.ntropy/config.toml`, the
passphrase-protected `.ntropy/identity.age` of an encrypted vault and `.git/`,
and, through a file symlink inside the vault, files outside it (established
with typst 0.15.1 on 2026-10-30). ADR 0070 accepts this today: the theme comes
from the vault itself, and typst cannot start a process.

## Context

The html and site exports refuse `.ntropy`, version-control directories,
secrets such as `.ssh` and `.env`, and anything outside the vault (ADR 0073).
None of that applies to what typst reads. Whether it should is a question of
how far a vault's own theme is trusted.

## Options

- Keep the residual as ADR 0070 states it.
- Give typst a narrower root: stage the document, the theme and the assets the
  note references into a temporary directory and compile there. Themes that
  reach other vault files by a vault-absolute path would break.
- Keep `--root` at the vault, and refuse to render when the theme or a file it
  imports is a symlink out of the vault. This closes only the symlink part.

## Open questions

- Do existing themes rely on reading arbitrary vault files, beyond their own
  directory and the note's assets?

## Relations

- Relates to: [Refuse note images that leave the vault through a symlink in PDF renders](01M66B2JM5XQDK7M3ZVKA1NZJV-refuse-note-images-that-leave-the-vault-through-a-symlink-in-pdf-renders.md)
