---
title: "Attest release binaries, pin the release toolchain and build musl in CI"
kind: chore
component: project
origin: review
tags: [security, ci]
---
# Attest release binaries, pin the release toolchain and build musl in CI

Three hardening steps for the release pipeline remain from the 2026-07-02 reviews of packaging, CI and the workflows' security. The release binaries carry no build provenance, they build with whatever stable Rust toolchain the runner has on the day, and the musl targets are first built during a release, after the GitHub release object exists.

## Build provenance attestation

The release uploads sha256 checksums next to each archive. The `upload-assets` job generates them with the same `contents: write` token it uploads the archives with, so whoever can replace an archive can replace its checksum too. `actions/attest-build-provenance` signs a provenance statement through GitHub's OIDC identity instead, which that token cannot forge.

It needs `id-token: write` and `attestations: write`. Grant them on the `upload-assets` job only, and pin the action to a commit SHA with its tag in a comment, like every other action in the workflows.

## Toolchain pin

No Rust toolchain is pinned: the repository has no `rust-toolchain.toml`. `Cargo.toml` declares `rust-version = "1.92"`, but that is only the oldest toolchain ntropy builds with, not the one a release uses. `upload-assets` builds through `taiki-e/upload-rust-binary-action`, which uses the `rustc` and `cargo` on the runner's path. A pin (a `rust-toolchain.toml`, or a `dtolnay/rust-toolchain` step with an explicit version in `upload-assets`) makes the release build reproducible. This is about reproducibility more than security.

## Musl build in the CI check

The release runs `ci.yml` before `create-release`, but CI builds only on `ubuntu-latest` (glibc) and `macos-latest`. The `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` binaries are first built in `upload-assets`, which runs after `create-release`, so a musl-only build break leaves a published release without its Linux assets. A musl build in `ci.yml` (for example `cargo build --target x86_64-unknown-linux-musl`) fails such a tag before the release object exists, and catches the break on `main` already.

## Done when

- `upload-assets` attests each archive, and `gh attestation verify` succeeds on a downloaded release archive.
- The release builds with a pinned Rust toolchain version, or the pin is explicitly declined.
- `ci.yml` builds at least one musl target, or the musl build in CI is explicitly declined.
