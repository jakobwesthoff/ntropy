# 75. Version numbers cover the CLI, not the library

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0013](0013-library-binary-split-with-thiserror-and-anyhow.md),
which splits the crate into a library and a binary.

## Context

The `ntropy` crate on crates.io holds both the library and the `ntropy`
binary. The library exists so that the business logic lives in one place
and further front ends, such as a mobile app or a desktop app, can use it
alongside the CLI. Nothing outside this repository is meant to build on it
yet, and its public items change whenever the CLI needs them to.
[ADR 0039](0039-vault-seed-content-as-embedded-files.md) already treated a
moved library constant as no versioned change for that reason.

## Decision

The library is an internal way to structure the code, not a public
interface. ntropy's version numbers describe the CLI release, and semantic
versioning applies to the CLI alone. A change to the library's public items
alone never raises the major version.

## Consequences

The library's API can change in any release, a patch release included.
Code that uses the crate as a library has to pin an exact version. The
crate documentation asks anyone who needs stable library versions to get in
touch; a stable library API is likely at some point, and such a request
brings it forward.
