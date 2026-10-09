---
title: "Release workflow publishes binaries without running the checks"
kind: bug
component: project
origin: review
tags: [ci]
---
# Release workflow publishes binaries without running the checks

Found in the 2026-07-02 codebase review of packaging and CI. `.github/workflows/release.yml` runs on `v*` tags and creates the GitHub release and uploads binaries straight away. Nothing in it runs `just check` (clippy, tests and fmt) first, and tag pushes do not trigger `ci.yml`, which runs only on pushes to `main` and on pull requests. A tag on a commit whose CI failed, or one that never ran CI, ships release binaries with no verification.

## Problem

Read in the code during the review. The musl targets are never exercised anywhere. CI runs only `ubuntu-latest` (glibc) and `macos-latest`, so a musl-only build break surfaces mid-release, after the GitHub release object already exists.

## Impact

A broken or unverified tag produces a GitHub release with uploaded binaries before anything has been checked.

## Suggested fix

- Add a `check` job at the top of `release.yml`, with the steps of `ci.yml` or reused through `workflow_call`, and make `create-release` depend on it. A broken tag then fails before any release artifact exists.
- Optionally add a musl build (`cargo build --target x86_64-unknown-linux-musl`) or the full check to CI, so musl breakage surfaces on `main` rather than at tag time.

## Done when

- A tag on a commit that fails `just check` creates no GitHub release and uploads no assets.
