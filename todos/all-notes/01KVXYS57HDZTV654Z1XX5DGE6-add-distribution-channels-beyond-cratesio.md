---
title: "Add distribution channels beyond crates.io"
kind: feature
component: project
status: needs-discussion
origin: discussion
---
# Add distribution channels beyond crates.io

Deferred during the v1 design (ADR 0022). Distribution is `cargo install` from
crates.io, plus prebuilt macOS and Linux binaries attached to tagged GitHub
releases by `.github/workflows/release.yml`.

## Open questions

- Should a shell or one-line installer script fetch the prebuilt release
  binaries?
- Should a Homebrew tap be added?
- Should a Nix flake be added?
