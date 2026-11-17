---
title: "Tests fail clippy when built without the encryption feature"
kind: refactor
component: cli
impact: low
origin: implementation
tags: [testing]
---
# Tests fail clippy when built without the encryption feature

Found on 2026-11-17 while the dependency update ran an extra check:
`cargo clippy --no-default-features --all-targets -- -D warnings` fails
with three dead-code errors. ntropy builds without its default
`encryption` feature (Cargo.toml), but neither the justfile nor CI checks
that build, so nothing catches it.

## Scope

`tests/cli.rs`, the helpers `run_show_on_terminal`, `stub_pager` and
`pager_env` (around line 8232) are only called from tests behind the
`encryption` feature, so a build without it reports them as unused. The
same lines are on `2138f58`; the failure predates the dependency update.
`cargo +1.92 clippy --no-default-features` shows the same three warnings.

## Done when

- The three helpers are gated with the tests that use them, or moved
  next to them, so the no-default-features clippy run is clean.
- A `just` recipe or CI step runs clippy (and the tests) with
  `--no-default-features`, so the build without encryption stays checked.

## Relations

- Relates to: [Decide whether to keep vault encryption](01M7MQZS5C380J2V8S7PJFSNA2-decide-whether-ntropy-keeps-vault-encryption.md), a build without encryption is what would remain
