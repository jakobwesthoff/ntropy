---
title: "Prune empty view directories during the sync walk"
kind: improvement
component: view
origin: idea
tags: [performance]
---
# Prune empty view directories during the sync walk

`sync_view`'s prune pass calls `fsutil::remove_dir_if_empty` on every directory
seen during the walk. `remove_dir_if_empty` issues its own `read_dir` to test
emptiness (`src/fsutil.rs`). So every sync, including a noop, reads every group
directory a second time, only to learn what the initial `collect_state` walk
already saw.

This overhead comes from the design choice to prune all walked directories. That
choice gives full within-view wipe parity, so a stray empty directory that
existed before the sync is also removed.

## Proposal

Reuse the walk. `collect_state` already enumerates every directory's contents,
so emptiness is known without extra reads:

- Track per-directory occupancy from the walk.
- Decrement the count as the diff removes leaves and stray files.
- `rmdir` directories that reach zero, cascading to parents bottom-up, with no
  second `read_dir`.

A directory that is empty during the walk (a pre-existing stray) starts at zero
and is pruned too, so the wipe-parity guarantee holds, entirely in memory.

The win is smaller than skipping `readlink`: hundreds of `readdir` calls rather
than about 10k `readlink`. It has no correctness cost.

## Open questions

- The walk's directory and leaf relationships must be kept, as a small tree or a
  parent-pointer map, instead of the current flat `BTreeSet<PathBuf>`.
- `fsutil::remove_dir_if_empty` may become unused. Remove it if so.

## Relations

- Relates to: [Skip link target checks in the post-mutation view sync](01KW4NTNVG8EVGS683HFME4M55-skip-link-target-checks-in-the-post-mutation-view-sync.md),
  same sync path. The two changes pair naturally.

## References

- `src/view/materialize.rs`: the `sync_view` prune loop, `collect_state`, and
  `actual_state` (the `DirSet`).
- `src/fsutil.rs`: `remove_dir_if_empty` (the redundant `read_dir`).
