# 19. Scan robustness and resource tolerance

Date: 2026-06-24

## Status

Accepted

## Context

Scanning `all-notes/` can meet files that are not well-formed notes: malformed
frontmatter, missing `title`, badly named `.md` files, or non-note files.
`all-notes/` may also legitimately hold resources (images, attachments),
including in subdirectories.

## Decision

Only top-level `*.md` files in `all-notes/` are notes. Everything else is
ignored silently: non-`.md` files and all subdirectories and their contents,
which are reserved for resources such as images.

A top-level `.md` file that does not parse, lacks a required `title`, or does
not match `<ulid>-<slug>` is skipped with a warning to stderr; the command
continues. stdout stays clean for piping.

`--strict` promotes those warnings to errors (non-zero exit) for validation and
CI use. The default is lenient (skip + warn).

## Consequences

- One malformed note never breaks a query.
- Resources can live alongside notes in `all-notes/` without noise or special
  configuration.
- The scan stays a non-recursive read of top-level `*.md`; subdirectories are
  never traversed for notes.
- `--strict` gives a "fail if the vault is dirty" check without changing the
  default behavior.
