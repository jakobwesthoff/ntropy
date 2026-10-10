---
title: "Ignore the generated site output and fill in the crate metadata"
kind: chore
component: project
origin: review
---
# Ignore the generated site output and fill in the crate metadata

Found in the 2026-07-02 codebase review of packaging and CI. The generated website output shows up as untracked noise in `git status`, and the crate metadata has gaps. Its finding on internal files in the published crate is fixed by the `exclude` list in `Cargo.toml`.

## Scope

### Generated website output

The pages workflow and local generator runs write the site into `dist/`. Add `/dist/` to the root `.gitignore`. ntropy's view sync manages that file only for view entries, and user lines are preserved (`src/gitignore.rs`), so a hand edit is safe.

### Metadata

- `homepage` is `http://westhoffswelt.de`, plain HTTP. With a project page now on GitHub Pages (`docs/pages`), it probably should point there.
- There are no `keywords` or `categories`, which hurts crates.io discoverability. Candidates are `command-line-utilities`, and keywords such as `notes`, `markdown` and `zettelkasten`.
- There is no `rust-version`. Edition 2024 and let-chains in the code imply a recent toolchain. Declaring it turns a cryptic build failure into a clear Cargo error on older toolchains.
- There is no `readme` field. Cargo detects `README.md` automatically, so this is optional. Check that it renders on crates.io.

## Open questions

Which homepage to set, which MSRV to declare, and which keywords and categories to use are for the user to decide, or to decline.

## Done when

- `git status` is clean after a local pages build.
- The metadata fields are decided and set, or the user has explicitly declined them.
