# view-sync-perf: targeted per-note updates on the mutation path

## Status

Open (idea, not yet designed). Large, invasive — the highest-ceiling option.

## Category

`view-sync-perf` — optimizations to the incremental view sync
(`view::sync_view`, ADR 0008). See sibling `view-sync-perf-*` todos.

## Context

Every mutation (`new`, `today`, edit-on-exit, `delete`) calls
`reconcile::refresh_views`, which re-scans the whole vault and diffs every
configured view against its full on-disk tree. Both halves of a mutation's cost
— the ~45 ms note scan and the view-tree read-walk — are paid in full even
though a single mutation changes exactly one note.

This is "Design B" as built: stateless, always correct, but it does
whole-vault work for a one-note change. "Design A" (targeted updates) was
considered during planning and deferred in favor of B's simplicity and
statelessness.

## Idea

On the mutation path, the changed note is known, so sync only the groups that
note participates in (old ∪ new), skipping the full scan and full read-walk.
Disambiguation is per-group (`view::leaf::leaf_names`), so a note's blast radius
is confined to its own groups — the property that makes this correct.

`reconcile` stays the full whole-vault pass; only `refresh_views` goes targeted.

## Trade-offs / open questions

- Needs the note's OLD group membership, which means snapshotting its
  frontmatter BEFORE `$EDITOR` in the editor flow, and parsing a note BEFORE
  deletion in `delete`. Reintroduces the old-state-tracking that Design B
  deliberately avoided.
- `refresh_views` (targeted) and `reconcile` (full) diverge — two code paths to
  keep correct, versus today's single one.
- Biggest latency win of the `view-sync-perf` set (drops a mutation toward
  query-speed or below), but the most invasive and the most fragile.
- Only worth doing if mutation latency becomes a felt problem; at personal
  scale (ADR 0020) the current ~135 ms already feels instant.
- Still stateless (no persisted index) — it derives old/new groups from the
  note itself, not from a cache.

## References

- `src/reconcile.rs` — `refresh_views` (mutation path) vs `reconcile` (full).
- `src/bin/ntropy/run/mod.rs` — `open_and_refresh` (editor flow; would need the
  pre-edit snapshot).
- `src/ops/delete.rs` — `delete_note` (would need to parse before removing).
- `src/view/leaf.rs` — per-group disambiguation (bounds the blast radius).
- ADR 0002 (stateless scan — this stays within it; no persisted index).
