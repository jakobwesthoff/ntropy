# `atomic_write` provides atomicity but not durability (no fsync)

## Severity

Low-priority robustness. Only matters across power loss / hard crash.

## Problem

`src/fsutil.rs:72-91` implements write-then-rename:

```rust
std::fs::write(&tmp_path, contents)...;
std::fs::rename(&tmp_path, path)...;
```

The doc comment promises "a reader either sees the old file or the fully
written new one, never a half-written note". That holds against concurrent
readers, but not against power loss: without an `fsync` on the temp file
before the rename (and on the parent directory after), filesystems with
delayed allocation may commit the rename before the data blocks, leaving a
truncated or zero-length destination after a crash. Every note write,
config write and `.gitignore` write in the crate funnels through this
function, so the blast radius is "the note you just saved".

## Suggested fix

Open the temp file explicitly, write, call `File::sync_all()` before the
rename; optionally `File::open(parent)` + `sync_all()` afterwards for the
directory entry. Alternatively adopt the `tempfile` crate's
`NamedTempFile::persist` plus an explicit sync (the crate is already a
dev-dependency; it would move to a regular dependency).

## Trade-off to decide

fsync per write costs milliseconds and this is a CLI writing small files on
explicit user action, so the cost is negligible; the main question is
whether crash-durability is considered in scope for v1 at all. If the
decision is "not in scope", downgrade the doc comment on `atomic_write` so
it doesn't over-promise.
