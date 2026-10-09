---
title: "Directory cleanup fails with ENOTEMPTY when a concurrent sync recreates an entry"
kind: bug
component: vault
origin: review
tags: [concurrency]
---
# Directory cleanup fails with ENOTEMPTY when a concurrent sync recreates an entry

Found in the 2026-07-02 codebase review of views and reconcile. The question was carried forward as open from an earlier review pass. `remove_dir_if_empty` in `src/fsutil.rs` (around lines 118 to 132) checks whether a directory is empty and then removes it. A concurrent process can add an entry between the check and the removal, and the removal then fails and aborts the whole sync, although the on-disk state is correct.

## Problem

Read in the code during the review. The function reads the directory, returns early if an entry exists, and then calls `std::fs::remove_dir`. Only `NotFound` is tolerated on removal.

ntropy runs this concurrently in normal use: the LSP server syncs views on document save while a CLI command syncs the same view tree. View sync calls `remove_dir_if_empty` on each subdirectory it saw, deepest first (`src/view/materialize.rs`, around lines 81 to 83). If another process recreates a leaf inside a group directory in the race window, `remove_dir` fails with `ENOTEMPTY` (`ErrorKind::DirectoryNotEmpty`) and the sync aborts.

The same error can occur without a race on some network filesystems, where NFS silly-rename `.nfs*` files keep a directory non-empty.

## Impact

A view sync fails with an error although the directory legitimately holds an entry again.

## Suggested fix

Treat `DirectoryNotEmpty` like the existing `NotFound` tolerance in the `remove_dir` match arm. The emptiness check is advisory, and a directory that turned out non-empty is the no-op case the function's contract already describes: it stays in place without error. `ErrorKind::DirectoryNotEmpty` is stable since Rust 1.83. The initial emptiness pre-check becomes an optimization only, since it avoids attempting removal of populated directories, and it can stay.

## Done when

- `remove_dir_if_empty` returns `Ok(())` when `remove_dir` fails with `DirectoryNotEmpty`.
- A unit test covers it. Racing deterministically is hard, so the acceptable coverage is: a populated directory passed to `std::fs::remove_dir` yields `DirectoryNotEmpty`, and `remove_dir_if_empty` tolerates a directory that becomes populated.
