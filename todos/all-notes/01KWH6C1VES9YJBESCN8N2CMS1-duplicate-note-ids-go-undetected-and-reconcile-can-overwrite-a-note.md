---
title: "Duplicate note IDs go undetected, and reconcile can overwrite a note"
kind: bug
component: vault
impact: critical
origin: review
tags: [durability]
---
# Duplicate note IDs go undetected, and reconcile can overwrite a note

Found in the 2026-07-02 codebase review of reconcile, with the scan module re-checked. Nothing detects two note files that share one ULID. `scan::scan_notes_dir` in `src/scan.rs` (around lines 58 to 126) parses each file on its own, and both files reach `Scan::notes` with the same `note.id`. `reconcile` can then rename one of them over the other, which deletes a note's content.

## Problem

Read in the code during the review. Duplicates arise when a user copies a note file to fork it, for example `cp all-notes/<ULID>-plan.md all-notes/<ULID>-plan-b.md`. The ULID in the filename comes along, and every ntropy layer then misbehaves quietly.

### Reconcile overwrites a note

The realign loop (`src/reconcile.rs`, around lines 96 to 108) renames each drifted note to `<its-ULID>-<slug-of-title>.md` through `fsutil::rename`, which is `std::fs::rename`. On Unix that replaces an existing destination atomically. Two files with the same ULID and the same title canonicalize to the same filename, so the second rename overwrites the file the first one produced. One note's content is gone, and the report lists both renames as successes. The single-note `realign` used by the editor flow (`src/reconcile.rs`, around lines 162 to 180) has the same problem.

### Link resolution and views

`link::index` keeps the first note per ID (`src/link/mod.rs`, around lines 97 to 103), so links resolve to whichever duplicate sorts first, and body rewrites then point the inbound links of both files at one of them. Both notes also produce identical or colliding view leaf names.

## Impact

Silent data loss: a note's file is overwritten, and the sync report still claims success. Link targets are wrong for both files.

## Suggested fix

- In `scan_notes_dir`, after collecting notes, detect ID collisions and emit a `ScanWarning` for each file beyond the first, or for every involved file, for example "duplicate note id <ULID>, also used by <path>". ADR 0019 already sets up warnings for per-file problems, and a vault-level integrity problem fits the same channel.
- Make `reconcile` and `realign` refuse to rename onto a path that already exists. An existing destination is always a bug or a duplicate. `fsutil::rename` could gain a no-clobber variant, for example through `renameat2(RENAME_NOREXCHANGE)` or `link` plus `unlink`, or a plain pre-check that accepts the time-of-check gap as best effort.

## Open questions

Should duplicates also be excluded from `notes`? They currently pass into every downstream operation. This is settled while implementing.

## Done when

- A test with two files that share a ULID and a title shows that `reconcile` does not lose one file's content. Either the scan warns and skips, or the rename refuses to clobber.
- A test pins the scan warning for duplicate IDs.

## Relations

- Relates to: [View leaf names can collide across groups and silently drop a note](01KWH6C1VES9YJBESCN8N2CMS0-view-leaf-names-can-collide-across-groups-and-silently-drop-a-note.md), duplicate IDs also produce colliding leaf names.
