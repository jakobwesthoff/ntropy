# 7. Vault directory layout

Date: 2026-06-24

## Status

Accepted

## Context

The vault holds canonical notes and materialized view trees (ADR 0008). The
layout must separate canonical from derived files and give the scanner an
unambiguous target.

## Decision

Vault root holds well-known top-level directories, each a way to look at the
notes:

    <vault>/
      all-notes/        canonical note files: <ulid>-<slug>.md
      by-tag/           a materialized view (symlink tree)
      by-<field>/       further views, one dir per defined view
      .ntropy/          configuration / templates (use TBD)

`all-notes/` holds the real files and is the source of truth; every other view
dir holds only symlinks into it. Naming it `all-notes` makes it a sibling of
the views: every top-level dir is a projection, and `all-notes` is the
lossless one.

The scanner targets `all-notes/` directly and non-recursively. ntropy owns the
directories named by view definitions plus the reserved names `all-notes` and
`.ntropy`.

## Consequences

- Canonical and derived files are separated by construction; the scanner needs
  no exclusion list.
- View dirs are defined by config, not a fixed `views/` parent. Any top-level
  dir not named by a view definition or reserved name is outside ntropy's
  management.
- `all-notes` and `.ntropy` are reserved names a user must not use for views.
