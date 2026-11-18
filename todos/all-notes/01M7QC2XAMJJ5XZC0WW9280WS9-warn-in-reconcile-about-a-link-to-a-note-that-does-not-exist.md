---
title: "Warn in reconcile about a link to a note that does not exist"
kind: improvement
component: view
origin: implementation
---
# Warn in reconcile about a link to a note that does not exist

`ntropy reconcile` says nothing about a note link whose ULID names no
note in the vault, and `--strict` does not fail on it. A link to a
deleted note, or one with a mistyped ULID, stays broken without anyone
noticing, while `reconcile` already warns about every broken asset link.
Found on 2026-11-18 in a dry run of an agent skill that archives meeting
minutes with a "previous meeting" link to an earlier note.

## Problem

`src/link/mod.rs`, `rewrite_body`, skips a link whose ULID resolves to no
note (`notes.get(&link.id)` is `None`), and the test
`reconcile_leaves_dangling_links_untouched` in `src/reconcile.rs` pins
that the link stays as it is. Nothing reports it. Reproduced on
2026-11-18 with ntropy 2.14.1: a note holding
`[Gone](01M7J26H35Z21W76PRMJ4QD3GP-daily-dev-sync.md)` in a vault without
that ULID gives `reconcile` 0 warnings and exit 0 under `--strict`.

A link to a `.md` file without a ULID (`[Other](no-such-note.md)`) is
reported, but with the asset message "is outside `all-notes/_assets/`, so
ntropy does not manage it", which does not say that the note is missing.

## Goal

`reconcile` prints a `warning:` line for each note link whose ULID names
no note, with the linking note and the target as written, and
`--strict` fails on it like on the asset warnings. The link itself stays
untouched, as today.

## Open questions

- Should a `.md` link without a ULID get its own message ("links a note
  file that does not exist") instead of the asset wording?
- Should `--strict search` or `show` also report it for the notes they
  touch, or only `reconcile`? The language server design lists
  dangling-link diagnostics as a later feature
  (`docs/design/language-server.md`).
