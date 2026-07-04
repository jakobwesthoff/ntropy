# `Error::Fs` exposes `FsError`, which library consumers cannot name

Found during the 2026-07-02 codebase review (unit 15, lib API surface).

## Problem

The public error enum has a variant whose payload type is unreachable from
outside the crate:

- `src/lib.rs:20` declares the module as `pub(crate) mod fsutil;`.
- `src/fsutil.rs:25` declares `pub struct FsError` inside it.
- `src/error.rs:31` puts it in the public API:
  `#[error(transparent)] Fs(#[from] FsError)` on the public `Error` enum.

This is the "Voldemort type" pattern: `FsError` is `pub`, so the compiler
accepts it in a public interface without warning (rustc's
`private_interfaces` lint does not fire for pub-in-private types), but its
effective visibility is `pub(crate)`. No `pub use` re-export exists anywhere
(verified by grep across `src/`).

Consequences for an external user of the `ntropy` library:

- They can match `Error::Fs(e)` and use `e` through its trait impls
  (`Display`, `std::error::Error`), but cannot write the type `FsError` in
  any signature, struct field, or `let` annotation, and cannot construct it.
- The generated `impl From<FsError> for Error` is a public impl referencing
  a type the user cannot name.
- rustdoc for the crate renders a variant/`From` impl whose payload type has
  no linkable docs page.

All nine other `Error` variants wrap types from `pub` modules
(`src/error.rs:15-24` imports them from `config`, `datetime`, `id`, `note`,
`ops`, `query`, `scan`, `template`, `vault`), so `Fs` is the only unnameable
one. The Rust API guidelines checklist calls this out under C-STABLE /
"public dependencies of a stable crate are stable": every type appearing in
a public API should itself be reachable.

## Why `fsutil` is `pub(crate)`

`src/fsutil.rs:5-12` documents the module as the crate-internal single point
of filesystem access (ADRs 0008, 0020). Keeping the *functions* crate-private
is intentional; only the error type leaks.

## Suggested resolution

Re-export just the type, keeping the module private, e.g. in `src/error.rs`
or `src/lib.rs`:

```rust
pub use crate::fsutil::FsError;
```

Alternatively wrap/convert the payload into a public type. Re-export is the
minimal change; the struct's fields are already private
(`src/fsutil.rs:26-29`) so nothing else escapes.

## Acceptance

- External code (e.g. a doctest or `tests/` integration test, which builds
  as an external crate) can write `fn f(e: ntropy::<path>::FsError)` and
  exhaustively handle `Error::Fs(e)` with the payload bound to a nameable
  type.
