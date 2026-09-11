---
title: Development
tags: [docs/develop]
site:
  index: true
  label: Development
  order: 6
---
This section is for people who build ntropy from source or want to know
how it is put together. This page covers the build, the `just` recipes,
the site frontend, and the snapshot tests. The pages under it are:

- [Limitations](01M28Q32Q4M6KYH4YXJ3EPDHQT-limitations.md): what ntropy
  does not do, and why.
- [Design](01M28Q32QQ6NJ4P58R1A3X6ZDD-design.md): the design documents and
  decision records, with a link to each.

ntropy is licensed under the Mozilla Public License 2.0
([`LICENSE`](https://github.com/jakobwesthoff/ntropy/blob/main/LICENSE)),
and every source file starts with the MPL header comment, as the
repository's `CLAUDE.md` requires.

## Building from source

```bash
git clone https://github.com/jakobwesthoff/ntropy.git
cd ntropy
cargo build --release
cargo install --path .   # install your working copy onto your PATH
```

## Recipes

Common tasks are wrapped as [`just`](https://github.com/casey/just)
recipes. `just` on its own, or `just --list`, prints them:

```bash
just test           # cargo test
just clippy         # cargo clippy --all-targets -- -D warnings
just fmt            # cargo fmt
just check          # clippy + tests + cargo fmt --check (the CI gate)
just coverage       # cargo llvm-cov
just verify-render  # the ignored tests that need the real typst binary
just bench          # scripts/benchmark.sh against a generated vault (needs hyperfine)
just site-install   # bun install --frozen-lockfile in site/
just site-build     # build the site frontend into src/site/dist/
just site-test      # type-check, lint, format-check, and test the frontend
just site-check     # site-build + site-test + git diff on src/site/dist/ (the CI gate)
```

CI runs `just check` and `just site-check`.

`just verify-render` runs the tests that need a `typst` binary on the
`PATH`. Its pdf, png, and typ artifacts land under `target/verify-render/`
for a look by eye.

## The site frontend

The browser-side code of the
[site export](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md) lives in
`site/` and is built with [Bun](https://bun.sh). `just site-build` writes
the result to `src/site/dist/`, which is committed. `cargo build` never
runs Bun: the build script embeds whatever is in `src/site/dist/` into the
binary. `just site-check` fails when the committed output no longer
matches the frontend sources, so rebuild and commit `src/site/dist/`
together with any change under `site/`.

## Snapshot tests

Tests use [`insta`](https://insta.rs) snapshots across all layers: unit
tests of the domain logic, integration tests over temporary vaults, and
CLI contract tests that run the real binary and snapshot stdout, stderr,
and the exit code
([ADR 0021](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0021-testing-strategy-with-insta-across-all-layers.md)).
When a change alters output, the snapshot assertions fail and leave
`.snap.new` or `.pending-snap` files next to the snapshots, which git ignores.
Review and accept them with
[`cargo-insta`](https://insta.rs/docs/cli/):

```bash
cargo insta review   # interactively accept/reject pending snapshots
cargo insta accept   # accept all pending snapshots
```
