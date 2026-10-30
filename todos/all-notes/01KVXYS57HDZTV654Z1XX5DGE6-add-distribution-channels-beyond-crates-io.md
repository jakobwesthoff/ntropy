---
title: "Add distribution channels beyond crates.io"
kind: feature
component: project
status: needs-discussion
horizon: someday
origin: discussion
---
# Add distribution channels beyond crates.io

Deferred during the v1 design (ADR 0022). Distribution is `cargo install` from
crates.io, prebuilt macOS and Linux binaries attached to tagged GitHub
releases by `.github/workflows/release.yml`, and a Homebrew formula in the
`jakobwesthoff/homebrew-tap` tap (`brew install jakobwesthoff/tap/ntropy`,
documented in the README).

## Open questions

- Should a shell or one-line installer script fetch the prebuilt release
  binaries?
- Should a Nix flake be added?

## Revisit when

No trigger known; reconsider at the next sweep.
