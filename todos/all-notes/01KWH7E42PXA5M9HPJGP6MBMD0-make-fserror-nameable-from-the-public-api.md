---
title: "Make FsError nameable from the public API"
kind: improvement
component: vault
origin: review
tags: [api-design]
---
# Make FsError nameable from the public API

Found in the 2026-07-02 codebase review of the library's public API. The public `Error` enum has a variant, `Fs`, whose payload type `FsError` no external crate can name. Every other `Error` variant wraps a type from a public module.

## Problem

Read in the code during the review. `pub(crate) mod fsutil;` in `src/lib.rs` (around line 20) hides `FsError`, which is declared `pub` in `src/fsutil.rs`. `src/error.rs` (around line 31) puts it into the public `Error` enum as `#[error(transparent)] Fs(#[from] FsError)`. This is the "Voldemort type" pattern. The compiler accepts it without a warning, because the `private_interfaces` lint does not fire for `pub` types in private modules, and no `pub use` re-export exists anywhere in `src/`.

External users can match `Error::Fs(e)` and use `e` through its trait impls (`Display`, `std::error::Error`). They cannot write the type in any signature, field or annotation, or construct it. The generated `From<FsError> for Error` impl is public and refers to a type users cannot name, and rustdoc shows a variant whose payload has no docs page. The other nine variants wrap types from public modules (`src/error.rs`, around lines 15 to 24). The Rust API guidelines checklist expects every type in a public API to be reachable.

## Goal

External users of the `ntropy` library can name `FsError` and match `Error::Fs(e)` with a payload of a type they can write.

## Proposal

Re-export only the type, keeping the module private, for example in `src/error.rs` or `src/lib.rs`:

```rust
pub use crate::fsutil::FsError;
```

Alternatively, wrap or convert the payload into a public type. Re-export is the minimal change, and the struct's fields are already private (`src/fsutil.rs`, around lines 26 to 29), so nothing else escapes. The functions stay crate-private. `src/fsutil.rs` (around lines 5 to 12) documents the module as the crate's single point of filesystem access (ADRs 0008 and 0020), and only the error type leaks.

## Done when

- External code, such as a doctest or an integration test in `tests/`, can name `FsError` through the re-export path and match `Error::Fs(e)` with that type.
