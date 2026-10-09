# view-sync-perf: prune empty directories from the walk, not a second readdir

## Status

Open (idea, not yet designed).

## Category

`view-sync-perf` — optimizations to the incremental view sync
(`view::sync_view`, ADR 0008). See sibling `view-sync-perf-*` todos.

## Context

`sync_view`'s prune pass calls `fsutil::remove_dir_if_empty` on **every**
directory seen during the walk, and `remove_dir_if_empty` issues its own
`read_dir` to test emptiness (`src/fsutil.rs`). So every sync — including a
noop — re-reads every group directory a second time, purely to discover what
the initial `collect_state` walk already saw.

This is self-inflicted overhead introduced when the design chose "prune all
walked dirs" for full within-view wipe parity (so a pre-existing stray empty
directory is also removed).

## Idea

Reuse the walk. `collect_state` already enumerates every directory's contents,
so emptiness is known for free:

- Track per-directory occupancy from the walk.
- Decrement as the diff removes leaves/stray files.
- `rmdir` directories that reach zero, cascading to parents bottom-up, with no
  second `read_dir`.

A directory empty during the walk (pre-existing stray) starts at zero and is
pruned too, so the wipe-parity guarantee is preserved — entirely in memory.

## Trade-offs / open questions

- Smaller win than the `readlink` change (hundreds of `readdir` calls, not
  ~10k `readlink`), but it is pure overhead with no correctness cost.
- Needs the dir/leaf relationships from the walk kept around (a small tree or
  parent-pointer map) rather than the current flat `BTreeSet<PathBuf>`.
- `fsutil::remove_dir_if_empty` may become unused afterward — remove it if so.
- Pairs naturally with `view-sync-perf-presence-only-trusted-sync`.

## References

- `src/view/materialize.rs` — `sync_view` prune loop, `collect_state`,
  `actual_state` (the `DirSet`).
- `src/fsutil.rs` — `remove_dir_if_empty` (the redundant `read_dir`).
