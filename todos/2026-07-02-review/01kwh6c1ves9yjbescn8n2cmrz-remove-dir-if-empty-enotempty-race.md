# `remove_dir_if_empty` races concurrent syncs: ENOTEMPTY surfaces as a hard error

Found during the 2026-07-02 codebase review (unit 07, views/reconcile;
carried forward as an open question from unit 03).

## Problem

`fsutil::remove_dir_if_empty` (`src/fsutil.rs:118-132`) implements
check-then-act: it reads the directory, returns early if any entry exists,
then calls `std::fs::remove_dir`. Only `NotFound` is tolerated on the
removal; any other error propagates.

Between the emptiness check and `remove_dir`, another process can create an
entry in the directory. ntropy has exactly this concurrency in normal use:
the LSP server (`ntropy lsp`) syncs views on document save while a CLI
command (e.g. `ntropy reconcile`, or any mutation that refreshes views) syncs
the same view tree. View sync calls `remove_dir_if_empty` on every
subdirectory it saw, deepest-first (`src/view/materialize.rs:81-83`). If the
other process recreates a leaf inside a group directory in the race window,
`remove_dir` fails with `ENOTEMPTY` (`ErrorKind::DirectoryNotEmpty`) and the
whole sync aborts with an error, even though the on-disk state is fine (the
directory legitimately holds an entry again).

The same error can also occur without any race on some network filesystems
(NFS silly-rename `.nfs*` files).

## Suggested fix

Treat `DirectoryNotEmpty` like the existing `NotFound` tolerance in the
`remove_dir` match arm: the emptiness check is advisory, and "directory
turned out non-empty" is exactly the no-op case the function's contract
describes ("a non-empty directory ... left in place without error").
`ErrorKind::DirectoryNotEmpty` is stable since Rust 1.83.

Note: the initial `read_dir`+`next()` emptiness pre-check then becomes an
optimization only (it avoids attempting `remove_dir` on populated dirs); it
can stay.

## Acceptance

- `remove_dir_if_empty` returns `Ok(())` when `remove_dir` fails with
  `DirectoryNotEmpty`.
- A unit test simulating the condition (e.g. calling the raw `remove_dir`
  branch behavior by racing is hard to test deterministically; testing that
  a populated dir passed straight to `std::fs::remove_dir` yields
  `DirectoryNotEmpty` and that `remove_dir_if_empty` tolerates a dir that
  becomes populated is acceptable coverage).
