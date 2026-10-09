# Duplicate note IDs are never detected; reconcile can silently overwrite a note file

Found during the 2026-07-02 codebase review (unit 07, reconcile; scan.rs
re-checked).

## Problem

Nothing in the codebase detects two note files sharing one ULID.
`scan::scan_notes_dir` (`src/scan.rs:58-126`) parses each `.md` file
independently and never cross-checks IDs; both files land in `Scan::notes`
with equal `note.id`.

How duplicates arise in practice: a user copies a note file to fork it
(`cp all-notes/<ULID>-plan.md all-notes/<ULID>-plan-b.md`) — the ULID in the
filename comes along. Every ntropy layer then misbehaves quietly:

1. **Data loss in `reconcile`** (the serious one). The realign loop
   (`src/reconcile.rs:96-108`) renames each drifted note to
   `<its-ULID>-<slug-of-title>.md` via `fsutil::rename`, which is
   `std::fs::rename` — on Unix it *replaces* an existing destination
   atomically. Two files with the same ULID and the same title both
   canonicalize to the same filename: the second rename overwrites the
   file the first one just produced. One note's content is gone, silently,
   and the report even lists both renames as successes. The same applies to
   the single-note `realign` used by the editor flow
   (`src/reconcile.rs:162-180`).
2. **Link resolution is first-match** — `link::index` keeps the first note
   per ID (`src/link/mod.rs:97-103`), so links resolve to whichever
   duplicate sorts first; body rewrites then point *both* files' inbound
   links at one of them.
3. **Views** — both notes produce identical or colliding leaf names; see
   `01kwh6c1ves9yjbescn8n2cms0-view-leaf-name-collisions-across-groups.md`
   Problem 3.

ADR 0019 (scan robustness) already establishes the warning mechanism for
per-file problems; duplicate IDs are a vault-level integrity problem that
fits the same reporting channel.

## Suggested resolution

- In `scan_notes_dir`, after collecting notes, detect ID collisions and emit
  a `ScanWarning` for each file beyond the first (or for all involved
  files), e.g. "duplicate note id <ULID>, also used by <path>". Decide
  whether duplicates should additionally be *excluded* from `notes` (they
  currently pass through into every downstream operation).
- Independently, make `reconcile`/`realign` refuse to rename onto a path
  that already exists (the destination existing is always a bug or a
  duplicate; `fsutil::rename` could grow a no-clobber variant, e.g. via
  `renameat2(RENAME_NOREXCHANGE)`/`link`+`unlink`, or a plain pre-check
  accepting the TOCTOU window as best-effort).

## Acceptance

- A test: two files with the same ULID and same title; `reconcile` must not
  end with one file's content lost. Either the scan warns and skips, or the
  rename refuses to clobber.
- A test pinning the scan warning for duplicate IDs.
