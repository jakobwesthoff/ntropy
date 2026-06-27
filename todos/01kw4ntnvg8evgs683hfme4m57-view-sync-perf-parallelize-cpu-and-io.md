# view-sync-perf: parallelize the per-view sync (CPU + I/O) with rayon

## Status

Approved to try. User accepted adding `rayon` as a dependency: implement,
test, benchmark, and revert if it does not earn its place.

## Category

`view-sync-perf` — optimizations to the incremental view sync
(`view::sync_view`, ADR 0008). See sibling `view-sync-perf-*` todos.

## Context

A single view's sync (`sync_view` in `src/view/materialize.rs`) has two
independent halves, both currently sequential:

- **Desired-link computation** (`desired_links`) — CPU-bound. For ~10k leaves it
  runs `slug::slugify(title)`, `note.created_date()`, per-group
  `leaf::leaf_names` disambiguation, and `fsutil::relative_path` per leaf.
- **Actual-state read** (`actual_state` / `collect_state`) — syscall-bound. One
  `readdir` per directory and **one `readlink` per leaf** (~10k `readlink` for a
  few-thousand-note vault — the single largest sync-specific syscall cost).

The original framing of this todo ("sync views in parallel") was the **wrong
axis**: the work is ~10k leaves, so splitting it across a handful of views
leaves most cores idle. The right unit of parallelism is the leaf/group, not the
view.

## What to parallelize (three pieces, one pass)

1. **`desired_links` across groups.** The group is the unit of independence:
   disambiguation is within-group (`leaf::leaf_names`), and each group writes a
   disjoint subdirectory, so groups produce conflict-free outputs. Keep the cheap
   group-by serial; `par_iter` the per-group leaf construction; flatten into the
   `BTreeMap`. The result is path-keyed, so output is deterministic regardless of
   completion order — snapshots/tests are unaffected.
2. **Actual readlinks (two-phase).** A `readdir` walk collects leaf paths (plus
   stray files and the dir set), then `par_iter` the symlink paths through
   `fsutil::read_link`.
3. **The `readdir` walk itself — only for high-cardinality views.** `by-tag` /
   `by-status`: a few hundred dirs vs ~10k leaves, so readdirs are negligible and
   a serial walk is fine. `by-codename` (the high-card view in
   `scripts/benchmark.sh`): ~one leaf per group, so readdirs ≈ readlinks and the
   serial walk becomes a real serial slice. If profiling shows it matters, make
   the walk parallel too (parallel recursive descent, or reuse the `ignore`
   crate's parallel walker — already a dependency — readlinking in the per-entry
   closure).
4. **Overlap CPU and I/O.** The two halves are independent, so wrap them in one
   `rayon::join(|| desired_links(...), || actual_state(...))` so the slugify /
   relative-path CPU runs while the readlinks are in flight.

## Why rayon (supersedes the earlier `std::thread::scope` / `ignore`-only ideas)

Once the **per-leaf CPU** is also parallelized, rayon is the single tool that
covers all of it: data-parallel CPU map (`par_iter` over groups), data-parallel
readlinks (`par_iter` over leaves), and the CPU/IO overlap (`join`).
`std::thread::scope` would mean hand-chunking three different things; the
`ignore` walker only parallelizes the directory read, not the CPU. That is the
justification for the dependency — "parallelize readlinks" alone was not.

## Path applicability (important sequencing)

- `desired_links` runs on **every** sync (mutation and reconcile) — you cannot
  skip computing what the tree should hold — so parallelizing the per-leaf CPU
  helps the hot **mutation** path too.
- Readlink parallelization helps only the **reconcile / verify** path: once
  `view-sync-perf-presence-only-trusted-sync` lands, the mutation path skips
  readlinks entirely, and "don't do the work" beats "do it in parallel." So
  sequence presence-only first; this change then parallelizes whatever the verify
  path still has to read.

## Stays serial (for now)

The diff (removal/creation) and the prune pass — a no-op sync has no writes, and
directory pruning is its own optimization (`view-sync-perf-prune-from-walk`).

## Thread-safety (confirmed)

`Note` and `Vault` are plain owned data (`Id`, `String`, `Vec`, `Mapping`,
`PathBuf`, `Option<SystemTime>` / a `Layout`+`PathBuf`) with no
`Rc`/`RefCell`/`Cell`, so `&Note` / `&Vault` / `&ViewDef` are `Sync` and freely
shareable across rayon threads. `sync_view` only does stateless syscalls into
each view's disjoint subtree, so there is no shared mutable state.

## Implementation sketch

```rust
let (desired, actual) = rayon::join(
    || desired_links(&view_dir, view, notes),  // par over groups
    || actual_state(&view_dir),                // par over leaf readlinks
);
let desired = desired?;
let (actual, dirs) = actual?;
```

- `desired_links`: serial group-by → `groups_vec.into_par_iter().map(group_leaves)
  .collect::<Result<Vec<_>>>()?` → flatten into the `BTreeMap` (disjoint keys, so
  the merge is conflict-free; `created_date()?` threads through the `Result`).
- `actual_state`: serial `readdir` walk collecting symlink paths / stray files /
  dirs → `symlinks.into_par_iter().map(read_link).collect::<Result<Vec<_>>>()?`.

## Open questions / steps

- **Profile first.** Measure the desired-CPU, `readdir`, and `readlink` slices on
  the 3000-note, 4-view benchmark, before and after. The ~45 ms note scan is
  already parallel and is the bigger floor; confirm each piece here is actually
  worth parallelizing.
- Decide whether to also parallelize across views (nested rayon under `sync_all`)
  or rely on within-view parallelism saturating cores (likely the latter — avoid
  oversubscription).
- Validate determinism with the existing snapshot/integration tests.
- Benchmark; if the win is marginal versus the dependency, revert.

## References

- `src/view/materialize.rs` — `sync_view`, `desired_links`, `actual_state`,
  `collect_state`.
- `src/view/leaf.rs` — `leaf_names` (within-group disambiguation that bounds
  independence to the group).
- `src/fsutil.rs` — `read_link`, `read_dir_entries`.
- `src/scan.rs` — the existing `ignore` parallel walker (no-dep alternative for a
  fully-parallel directory walk).
- Sibling: `view-sync-perf-presence-only-trusted-sync` (sequence first),
  `view-sync-perf-prune-from-walk`.
