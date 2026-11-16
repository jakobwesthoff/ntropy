---
title: "Accept a note's filename or path wherever a command takes an ID or query"
kind: feature
component: vault
status: needs-discussion
origin: request
tags: [ux]
---
# Accept a note's filename or path wherever a command takes an ID or query

User feedback from 2026-11-16: a script that renders a note wants a
reference that cannot start to match a second note later. A path, as
`search -p` prints it, would be one, but `render`, `show`, `search` and
`delete` take only a full ULID or a DSL query. `write` already takes a
ULID, a filename or a path in `all-notes/` (ADR 0043), so the commands
disagree on what names a note.

## Goal

Every command that takes `[ID|QUERY]` also takes a note's filename or its
path. A path inside the vault's `all-notes/` selects that note. A path to
a file outside the vault is an error that says the file is not a note of
this vault, rather than being run as a query that matches nothing.

## Proposal

Extend `resolve_selection` in `src/ops/select.rs`, which `search`,
`delete` and `select_one` (`render`, `show`) share, with the
filename-or-path branch of `src/ops/write.rs`, and move that logic to one
place both use.

## Open questions

- How to tell a path from a query. `write` treats any separator as a
  path, but queries contain `/` as well (`tag:ekko/meeting`), so that
  rule cannot carry over as it is. Candidates: an existing file wins, a
  `.md` suffix, or a leading `./`, `/` or `~`.
- Whether a filename alone (no separator) is accepted, as `write` does,
  or only a full path.
