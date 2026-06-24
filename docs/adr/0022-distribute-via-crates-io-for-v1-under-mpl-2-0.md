# 22. Distribute via crates.io for v1 under MPL-2.0

Date: 2026-06-24

## Status

Accepted

## Context

ntropy needs a v1 distribution channel and a license. It targets Unix (ADR
0020).

## Decision

v1 is distributed via crates.io only (`cargo install ntropy`). Prebuilt-binary
releases (e.g. `dist`/cargo-dist), a Homebrew tap, and Nix packaging are
deferred.

The license is MPL-2.0 (`LICENSE`). Every source file must carry the MPL-2.0
header comment (recorded in the project `CLAUDE.md`).

Cargo metadata is set for publishing: `authors`, `description`, `homepage`,
`repository`, `license`.

## Consequences

- Installation requires a Rust toolchain in v1; no no-toolchain binaries yet.
- The crate must stay publishable (valid metadata, no path-only deps).
- File-level MPL headers are a standing requirement for all source.
