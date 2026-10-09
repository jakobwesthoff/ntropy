# Release workflow builds and publishes binaries without running the test suite

Found during the 2026-07-02 codebase review (unit 14, packaging & CI).

## Problem

`.github/workflows/release.yml` triggers on `v*` tags and immediately
creates the GitHub release (`taiki-e/create-gh-release-action`) and builds
and uploads binaries for four targets. Nothing in the workflow runs
`just check` (clippy + tests + fmt) first, and tag pushes are not gated on
the CI workflow (`ci.yml` runs on `push: branches: [main]` and PRs — a tag
push does not run it).

A tag created on a commit whose CI run failed (or that never ran on CI,
e.g. tagged before pushing the branch, or on a non-main commit) ships
release binaries with zero verification. The musl targets additionally are
never exercised anywhere: CI runs only ubuntu-latest (gnu) and
macos-latest, so a musl-only build break is discovered mid-release, after
the GitHub release object already exists.

## Suggested resolution

- Add a `check` job at the top of release.yml (same steps as ci.yml, or
  `workflow_call` reuse) and make `create-release` depend on it, so a
  broken tag fails before any release artifact exists.
- Optionally add `cargo build --target x86_64-unknown-linux-musl` (or the
  full check) to CI so musl breakage surfaces on main, not at tag time.

## Acceptance

- A tag on a commit that fails `just check` produces no GitHub release and
  no uploaded assets.
