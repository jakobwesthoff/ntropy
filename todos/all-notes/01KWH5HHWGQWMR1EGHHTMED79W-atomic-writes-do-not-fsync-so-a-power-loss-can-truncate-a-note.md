---
title: "Atomic writes do not fsync, so a power loss can truncate a note"
kind: bug
component: vault
origin: review
tags: [durability]
---
# Atomic writes do not fsync, so a power loss can truncate a note

Found in the 2026-07-02 codebase review of the filesystem utilities. `atomic_write` in `src/fsutil.rs` gives atomicity against concurrent readers, but it never calls `fsync`. After a power loss or hard crash, the rename can reach the disk before the file data, which leaves a truncated or zero-length file.

## Problem

Read in the code during the review. `atomic_write` (around line 72) writes a temporary file and renames it over the destination:

```rust
std::fs::write(&tmp_path, contents)...;
std::fs::rename(&tmp_path, path)...;
```

The doc comment promises that "a reader either sees the old file or the fully written new one, never a half-written note". That holds for concurrent readers. It does not hold across a power loss: without an `fsync` on the temporary file before the rename, and on the parent directory after it, filesystems with delayed allocation may commit the rename before the data blocks.

## Impact

Every note write, config write and `.gitignore` write goes through this function. A crash at the wrong moment leaves the note just saved truncated or empty.

## Suggested fix

Open the temporary file explicitly, write it, and call `File::sync_all()` before the rename. Optionally call `sync_all()` on the parent directory afterwards to persist the directory entry. The alternative is the `tempfile` crate's `NamedTempFile::persist` with an explicit sync; the crate is already a dev-dependency and would move to a regular one. The cost of the sync is not known yet: the measurement below decides whether it is acceptable.

## Measurement

Measured 2026-10-30 on an Apple M4 Pro (macOS 27.0.1, APFS, internal SSD), release build of v2.4.0 with a variant switch, vaults of 30,000 and 50,000 generated notes (`examples/generate_vault.rs`, seed 305419896), hyperfine with interleaved variants.

- On Apple targets Rust's `File::sync_all` and `File::sync_data` both issue `fcntl(F_FULLFSYNC)`, which flushes the drive's cache. Plain `fsync(2)` (through `libc`) does not flush that cache on macOS, so it does not protect against power loss there. On Linux `sync_all` is `fsync`.
- Cost per `atomic_write`: +4.9 to 5.7 ms with `F_FULLFSYNC` on the file, +9.5 to 10.3 ms with file and directory, +0.01 to 0.06 ms with plain `fsync`.
- A single `new` or `write` in a plaintext vault: the added 5 to 10 ms disappears in the 1.2 s (30,000 notes) to 2.5 s (50,000 notes) of scan and view refresh. An encrypted `write`: 17.3 ms, 21.9 ms with a file flush, 27.2 ms with file and directory.
- Bulk: a `reconcile` retitling all 50,000 notes (25,069 link rewrites) takes 17.7 s today, 152 s with a file flush, 264 s with file and directory. With 10% retitled (2,442 rewrites): 3.8 s, 17.3 s, 28.1 s. `vault encrypt` of 30,000 notes: 33 s, 183 s, 319 s.
- `vault encrypt` already flushes every output after all writes (`sync_file` per file, one `sync_dir`), and that barrier is inside its 33 s baseline. A batched barrier at the end of a bulk operation is therefore far cheaper than a flush per write.
- `reconcile`'s note renames use plain `std::fs::rename`, not `atomic_write`, and are outside every variant.

The full report, with tables, intervals and the scripts, is kept outside the repository; the numbers above are its headline.

## Decisions

- 2026-10-30, user: yes, flush the file and its directory on ordinary note writes, but only after measuring the cost on a vault of 30,000 to 50,000 notes. The measurement comes first and decides whether the cost is acceptable.
