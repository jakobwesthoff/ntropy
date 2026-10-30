---
title: "CLI tests read the real global config, so host state breaks them"
kind: bug
component: cli
origin: review
tags: [testing]
---
# CLI tests read the real global config, so host state breaks them

Found in the 2026-07-02 codebase review of the tests and fixtures. Almost every CLI command loads the global config during vault resolution, from the OS-native path. The CLI contract tests spawn the real binary and never isolate that path, so their results depend on the machine they run on. No override exists to isolate them with.

## Problem

Read in the code during the review. `global::load()` is called during vault resolution (`src/bin/ntropy/run/mod.rs`, around lines 91 to 101). It reads the path from `directories::ProjectDirs` (`src/config/global.rs`, around lines 35 to 37): `~/Library/Application Support/ntropy/config.toml` on macOS and `~/.config/ntropy/config.toml` on Linux. The `ntropy()` helper in `tests/cli.rs` (around lines 47 to 54) clears only `NTROPY_VAULT`, `VISUAL` and `EDITOR`. This causes three problems:

1. Host state causes failures. A malformed or unreadable real config makes `global::load()` fail, and nearly every test then fails with "while loading the global config", whatever the code under test does.
2. Host-dependent output. `info` prints the host's default vault. The test hides this with a redaction filter on the whole line (`tests/cli.rs`, around line 426), so the "no default set" and "default set" variants are never pinned.
3. `init --set-default` cannot be tested. It writes to the real global config (`set_global_default`, `src/bin/ntropy/run/mod.rs`, around lines 373 to 380), so no test exercises it.

`config_path()` consults only `ProjectDirs`. That still leaves a way in from outside on every platform: `directories` 6.0 derives the macOS config directory from `$HOME` (`home_dir().join("Library/Application Support")`, with `dirs-sys` reading `HOME` first), and Linux honours `XDG_CONFIG_HOME`. The test `info_print_fails_without_a_vault` in `tests/cli.rs` already isolates itself this way by setting `HOME` and `XDG_CONFIG_HOME` to a temporary directory (checked in the `directories` 6.0.0 source, 2026-10-30).

## Suggested fix

Either point `HOME` and `XDG_CONFIG_HOME` at a per-test temporary directory in the `ntropy()` helper, which needs no code change but affects anything else in the binary that reads `HOME`, or add an ntropy-level override that `global::config_path()` consults before `ProjectDirs`, for example `NTROPY_CONFIG_DIR` (a directory) or `NTROPY_GLOBAL_CONFIG` (a file path), which adds a public environment variable. Then:

- isolate the config in the `ntropy()` helper in `tests/cli.rs`,
- pin both `Default vault: (not set)` and a set default in `info` snapshots, dropping the blanket redaction,
- add a contract test for `init --set-default` that checks the config is written at the overridden location.

If the override is chosen, document the variable next to `NTROPY_VAULT` (ADR 0016).

## Done when

- `cargo test` passes with a deliberately corrupt config at the real OS location, checked by hand, because the tests no longer read it.
- An `init --set-default` contract test exists.
