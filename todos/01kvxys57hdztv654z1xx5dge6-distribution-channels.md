# Distribution channels beyond crates.io (post-v1)

Deferred from v1 per ADR 0022. v1 ships via crates.io (`cargo install`) only.

## To decide later

- Prebuilt macOS/Linux binaries + a shell installer via `dist` (cargo-dist) on
  tagged GitHub releases.
- A Homebrew tap (dist can generate/update it).
- A Nix flake.
