# view-sync-perf: sync views in parallel

## Status

Open (idea, not yet designed).

## Category

`view-sync-perf` — optimizations to the incremental view sync
(`view::sync_view`, ADR 0008). See sibling `view-sync-perf-*` todos.

## Context

`view::sync_all` syncs configured views one after another
(`src/view/mod.rs`). Each view is fully independent — its desired projection,
read-walk, diff, and writes touch only its own directory subtree. The dominant
per-view cost is the read-walk (see
`view-sync-perf-presence-only-trusted-sync`), which is syscall-bound.

The note scan (ADR 0020) is already a parallel walk, but the view sync is not.
With the benchmark now configuring four views (`scripts/benchmark.sh`), the
sequential read-walks add up.

## Idea

Run the per-view syncs concurrently (e.g. a `rayon` parallel iterator over
views), overlapping their read-walks. Optionally parallelize the per-group
reads within a single large view as well. SSD/APFS handle concurrent
`readdir`/`readlink` well, so this should approach a near-linear speedup on the
read-walk for multi-view vaults.

## Trade-offs / open questions

- Adds a parallelism dependency/complexity to the `view` layer (check whether
  `rayon` or the existing scan's mechanism is already available).
- Views write into disjoint directory subtrees, so no cross-view contention;
  confirm the `.gitignore` sync stays sequential and after the parallel section
  (`sync_views_and_gitignore` in `src/reconcile.rs`).
- Benefit scales with view count; little gain for a single-view vault.
- Composes with the `readlink`-skipping and prune-from-walk todos (parallelize
  whatever the per-view cost has been reduced to).

## References

- `src/view/mod.rs` — `sync_all` (the sequential loop).
- `src/reconcile.rs` — `sync_views_and_gitignore` ordering.
- `scripts/benchmark.sh` — `reconcile-4-views` row measures the multi-view case.
- ADR 0020 (the note scan already uses a parallel walk).
