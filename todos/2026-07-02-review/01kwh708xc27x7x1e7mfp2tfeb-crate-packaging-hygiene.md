# Crate packaging hygiene: internal files ship to crates.io; metadata gaps; dist/ unignored

Found during the 2026-07-02 codebase review (unit 14, packaging & CI).

## Problem 1: internal working files ship in the published crate

`Cargo.toml` has no `exclude`/`include` list, so `cargo publish` packages
everything that is not gitignored. That includes:

- `todos/` — 20+ internal planning/todo files (tracked in git, verified via
  `git ls-files todos/`), including this review folder once committed;
- `docs/pages/`, `docs/vhs/` — website source and demo tooling;
- `.github/` — CI workflows.

ntropy distributes via crates.io (ADR 0022), so these land in every
`cargo install ntropy` download. None are needed to build or use the crate.

Fix: add an `include` list (src, tests, examples, LICENSE, README,
CHANGELOG, Cargo.toml) or an `exclude` list covering the above. `include`
is the safer allowlist form.

## Problem 2: `dist/` is generated output but not gitignored

The pages workflow (and local generator runs) emit the website into
`dist/`; it currently shows up as untracked noise in `git status` (visible
in this working tree). Add `/dist/` to the root `.gitignore`. Note the
repo's own `.gitignore` is auto-managed by ntropy's view sync only for
*view entries* — user lines are preserved (`src/gitignore.rs`), so adding
it by hand is safe.

## Problem 3: Cargo.toml metadata gaps

- `homepage = "http://westhoffswelt.de"` — plain HTTP, and now that a
  project page exists (docs/pages → GitHub Pages), it likely should point
  there instead.
- No `keywords`/`categories` — crates.io discoverability (e.g.
  `command-line-utilities`, keywords like `notes`, `markdown`, `zettelkasten`).
- No `rust-version` (MSRV) — edition 2024 plus `let`-chains in the code
  imply a recent toolchain; declaring it turns a cryptic build failure into
  a clear cargo error for older toolchains.
- No `readme` field (cargo auto-detects `README.md`, so this one is
  optional; verify it renders on crates.io).

## Acceptance

- `cargo package --list` shows no `todos/`, `docs/pages/`, `docs/vhs/`, or
  `.github/` entries.
- `git status` clean after a local pages build.
- Metadata fields decided and set (or explicitly declined by the user).
