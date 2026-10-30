# 79. Note writes are flushed to disk

Date: 2026-10-30

## Status

Accepted

## Context

Every write goes to a sibling temp file that is renamed over the destination.
The rename is atomic, so a concurrent reader sees the old file or the whole new
one. Nothing forces the temp file's data to disk before the rename, and a
filesystem with delayed allocation may persist the rename first. A power loss
at that moment leaves the note present but truncated or empty.

On Apple targets `File::sync_all` issues `fcntl(F_FULLFSYNC)`, which flushes
the drive's write cache. Plain `fsync(2)` there only hands the data to the
drive, so it does not survive a power loss. On Linux `sync_all` is `fsync`.

Flushing after every write makes each one wait for the drive. A command that
rewrites tens of thousands of notes would take minutes instead of seconds.

## Decision

A write through `fsutil` returns when its data is on stable storage:

1. the temp file is flushed with `File::sync_all`,
2. it is renamed over the destination,
3. the directory is flushed with `File::sync_all`.

Step 1 before step 2 keeps a destination name from ever pointing at data that
is not on disk. Step 3 makes the new name itself durable. Renames of existing
notes (`reconcile`, the editor and `write` realigning a filename, the rename of
a rekeyed note) flush the directories they change. The wrapped vault identity,
the config files and `.gitignore` take the same path.

A command that writes many notes opens a flush scope, writes everything, and
finishes the scope once. The scope keeps every written temp file unrenamed.
Finishing it flushes each temp file, renames them all into place, and flushes
each touched directory once. That is one flush per file and one per directory,
in the order of a single write, so a note is whole after a power loss at any
point of the batch. `reconcile` (renames and link rewrites) and the
conversions `vault encrypt`, `vault decrypt` and `vault rekey` use a scope.
A conversion also flushes its source removals and renames before it removes
its marker, and a resumed conversion flushes the targets it finds in place.

`reconcile` adopting plaintext notes into an encrypted vault writes and flushes
one note at a time, and flushes the directory after removing each plaintext
source. A batch would leave every source next to its durable encrypted twin
until it ended, and after a crash the next run reads each such pair as a copy
and gives the source a new ULID. A locked vault cannot compare contents to
tell the two apart.

A filesystem that answers a flush with `EINVAL`, `ENOTSUP` or `ENOTTY` gets no
durability and no error, so a vault on a mount that cannot flush keeps
accepting writes. Plain `fsync` is not tried as a fallback. Every other flush
error, such as `EIO` or `ENOSPC`, is returned.

## Consequences

- A flush error after a rename that happened is returned as an error without
  undoing the rename. The note is in place; whether it survives a power loss is
  unknown.
- A process killed during a batch leaves its temp files (`<name>.<ULID>.tmp`),
  up to one per written note. The scan ignores them, because only `.md` and
  `.age` names are notes, and nothing deletes them. The notes they would have
  replaced are intact. A cleanup keyed on the age of a temp file could delete
  the temp files of another ntropy process that is still inside its batch.
- Removals by `delete` and the view symlinks are not flushed. A removal lost to
  a power loss brings the note back; it never truncates one.
- The scope is per thread. A write on another thread is flushed at once and is
  not deferred.
