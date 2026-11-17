---
title: "A view takes over an existing directory and deletes its files"
kind: bug
component: view
status: needs-discussion
impact: high
origin: implementation
tags: [durability]
---
# A view takes over an existing directory and deletes its files

A view definition whose name matches a directory that already exists in the vault, such as `docs` or `src`, makes the next view sync delete every regular file in that directory. Two views whose names nest, such as `area` and `area/work`, delete each other's links on every sync. Both follow from ADR 0007, under which ntropy owns every directory a view definition names, and neither involves an invalid name.

## Problem

Found on 2026-10-30 while adding the validation that keeps view names inside the vault. Read in the code, not reproduced. `sync_view` in `src/view/materialize.rs` removes every entry of the view directory that is not one of the view's leaf symlinks, which is right for a directory ntropy created and wrong for one the user created. A user who adds `[[view]] name = "docs"` to a vault that already holds a `docs/` folder of their own loses its files without a prompt or a warning.

For nested names, the parent view treats the child view's directory as foreign content inside its own tree, and the child view's sync then recreates it.

The same removal hits a file saved inside a view tree after the view exists. Reproduced on 2026-11-17 in the final review of the asset system: a note opened through `by-tag/work/` and an editor that pastes an image next to it creates `by-tag/work/pasted-image.png`; the next `reconcile`, `new`, `write` or `asset add` deletes it without a warning, and `--strict` exits 0. The asset system keeps a real `_assets` directory in a view that holds files, because such a file may be the only copy (ADR 0100), and `vault encrypt` refuses while a view holds a file an editor saved through it (ADR 0103); a view sync deletes that same file in any other place of the tree before either gets to see it.

## Suggested direction for the saved-file case

Treat a regular file in a view tree like the kept `_assets` directory: leave it, report a warning that counts for `--strict`, and point to `asset add --move` to give it a home.

## Options

- Refuse to take over a directory ntropy did not create: mark view directories (a marker file, or a record in `.ntropy/`) and skip a view whose directory exists without the mark, with a warning that `--strict` turns into a failure.
- Refuse only when the existing directory holds regular files, which a view directory never does.
- Reject view names that nest inside another view's name at config load.

## Open questions

- Which of the options, or a combination: the marker is the most robust, but existing vaults have unmarked view directories that must keep working, so it needs a migration rule.
- Whether nesting is rejected or the parent view learns to leave a child view's directory alone.
