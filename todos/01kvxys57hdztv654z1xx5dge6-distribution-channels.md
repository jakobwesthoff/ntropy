# Distribution channels beyond crates.io (post-v1)

Deferred from v1 per ADR 0022. Distribution is via crates.io (`cargo install`)
plus prebuilt macOS/Linux binaries attached to tagged GitHub releases
(`.github/workflows/release.yml`).

## To decide later

- A shell/one-line installer script for the prebuilt release binaries.
- A Homebrew tap.
- A Nix flake.
