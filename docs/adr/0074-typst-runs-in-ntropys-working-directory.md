# 74. typst runs in ntropy's working directory

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0040](0040-custom-typst-engine-with-own-markdown-emitter.md),
which runs `typst` for the `pdf` format in the note's directory and gives
`Invocation` a working-directory field for it.

## Context

ADR 0040 runs `typst` in the note's directory so the note's relative asset
paths resolve beside the note. Since [ADR 0045](0045-vault-render-themes.md)
nothing typst reads for the document depends on that directory: the document
arrives on stdin and sits at the compile root, `--root` names the vault, the
emitter writes local image paths root-absolute, and the output path typst
receives is absolute. Compiled from the note's directory, from another
directory and from `/`, the same document gives byte-identical PDFs.

## Decision

`typst` runs in ntropy's own working directory. `Invocation` has no
working-directory field, and the binary spawns a tool without changing
directory, looking a bare program name up on `$PATH` from there. A spawn that
fails because the program is not found, not permitted, or behind a `$PATH`
entry that is not a directory reports the tool as unavailable, naming what to
install.

The binary still joins a relative `-o` path onto its working directory. The
vault-containment warning compares the absolute path with the vault root, and
typst would read a relative name starting with `-` as an option and a lone `-`
as standard output.

## Consequences

- A relative path in typst's own environment variables `TYPST_FONT_PATHS`,
  `TYPST_PACKAGE_PATH`, `TYPST_PACKAGE_CACHE_PATH` and `TYPST_CERT` resolves
  against the directory ntropy runs in. `TYPST_ROOT` plays no part, since
  ntropy always passes `--root`.
- With `$PATH` unset, `typst` is looked up in the C library's default path,
  not in the working directory.
