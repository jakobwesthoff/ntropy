---
title: "Global config is written without atomic_write"
kind: bug
component: vault
origin: review
tags: [durability]
---
# Global config is written without atomic_write

Found in the 2026-07-02 codebase review of the config module. The module docs of `src/fsutil.rs` state that every write, symlink, rename and directory read or removal goes through that module, and that no higher layer calls `std::fs` directly. `write_at` in `src/config/global.rs` (around line 55) breaks that invariant, and a crash during a config write can leave a `config.toml` that fails to parse on every later run.

## Problem

Read in the code during the review. `write_at` calls `std::fs::create_dir_all` and `std::fs::write` directly. This has two consequences:

1. The global config write is not atomic. All note writes go through `atomic_write`, and the config should use the same path. A truncated `config.toml` fails with `ConfigError::Parse` on every subsequent run.
2. The fsutil invariant is false as written. That misleads anyone auditing filesystem behaviour, which is the failure the invariant exists to prevent. The review found this only by reading the code.

`fsutil` is `pub(crate)` (`src/lib.rs`, around line 20), so `config` can use it directly.

## Impact

After a crash during a config write, every command that loads the global config fails with a parse error until the file is repaired by hand.

## Suggested fix

Route `write_at` through `fsutil::create_dir_all` and `fsutil::atomic_write`, mapping `FsError` into `ConfigError::Write` or letting the error type carry `FsError`. Then re-audit the remaining direct `std::fs` uses outside fsutil. Known ones at review time: `src/config/global.rs` (around lines 41, 57 and 63), `src/config/per_vault.rs` (around line 39), `src/scan.rs` (around lines 135 and 136) and `src/vault/resolve.rs` (around line 143). Reads are arguably fine, since the fsutil doc only claims writes, renames and directory operations; `global.rs` writes.

## Open questions

For each remaining direct use, either route it through fsutil or narrow the fsutil doc claim to what is true.

## Relations

- Relates to: [Atomic writes do not fsync, so a power loss can truncate a note](01KWH5HHWGQWMR1EGHHTMED79W-atomic-writes-do-not-fsync-so-a-power-loss-can-truncate-a-note.md), same write path; the fsync fix in `atomic_write` also covers config writes once they route through it.
