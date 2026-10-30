---
title: "Fill in the crate metadata"
kind: chore
component: project
origin: review
---
# Fill in the crate metadata

Found in the 2026-07-02 codebase review of packaging and CI. The crate metadata has gaps. The review's other findings are fixed: internal files in the published crate by the `exclude` list in `Cargo.toml`, and the generated website output by `/dist` in the root `.gitignore` (commit `8123223`).

## Metadata

- `homepage` is `http://westhoffswelt.de`, plain HTTP. With a project page now on GitHub Pages (exported from `docs/website`), it probably should point there.
- There are no `keywords` or `categories`, which hurts crates.io discoverability. Candidates are `command-line-utilities`, and keywords such as `notes`, `markdown` and `zettelkasten`.
- There is no `rust-version`. Edition 2024 and let-chains in the code imply a recent toolchain. Declaring it turns a cryptic build failure into a clear Cargo error on older toolchains.
- There is no `readme` field. Cargo detects `README.md` automatically, so this is optional. Check that it renders on crates.io.

## Open questions

Which homepage to set, which MSRV to declare, and which keywords and categories to use are for the user to decide, or to decline.

## Done when

- The metadata fields are decided and set, or the user has explicitly declined them.
