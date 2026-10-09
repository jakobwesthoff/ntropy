---
title: "Atomic writes do not fsync, so a power loss can truncate a note"
kind: bug
component: vault
status: needs-discussion
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

Open the temporary file explicitly, write it, and call `File::sync_all()` before the rename. Optionally call `sync_all()` on the parent directory afterwards to persist the directory entry. The alternative is the `tempfile` crate's `NamedTempFile::persist` with an explicit sync; the crate is already a dev-dependency and would move to a regular one. An fsync per write costs milliseconds, which is negligible for a CLI that writes small files on explicit user action.

## Open questions

Is crash durability in scope for ntropy at all? If not, the doc comment on `atomic_write` should stop promising more than the code delivers, and the todo closes without code changes.
