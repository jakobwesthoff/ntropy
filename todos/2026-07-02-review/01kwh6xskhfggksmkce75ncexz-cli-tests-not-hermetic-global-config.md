# CLI contract tests read the developer's real global config; no override hook exists

Found during the 2026-07-02 codebase review (unit 13, tests & fixtures).

## Problem

Almost every CLI command calls `global::load()` during vault resolution
(`src/bin/ntropy/run/mod.rs:91-101`), which reads the OS-native config path
from `directories::ProjectDirs` (`src/config/global.rs:35-37`): on macOS
`~/Library/Application Support/ntropy/config.toml`, on Linux
`~/.config/ntropy/config.toml`.

The CLI contract tests (`tests/cli.rs`) spawn the real binary and never
isolate that path — the `ntropy()` helper only clears `NTROPY_VAULT` and
`VISUAL`/`EDITOR` (`tests/cli.rs:47-54`). Consequences:

1. **Spurious failures from host state.** If the developer's (or CI
   machine's) real global config is malformed TOML or unreadable,
   `global::load()` errors and nearly every test fails with "while loading
   the global config", regardless of the code under test.
2. **Host-dependent output.** `info` prints the host's default vault; the
   test papers over this with a redaction filter for the whole line
   (`tests/cli.rs:426`), which also means the "no default set" and "default
   set" rendering variants are never actually pinned.
3. **`init --set-default` is untestable.** It writes to the developer's
   *real* global config (`set_global_default`,
   `src/bin/ntropy/run/mod.rs:373-380`), so no test exercises it — and none
   must, until isolation exists.

There is no environment override: `config_path()` consults only
`ProjectDirs`. On Linux `XDG_CONFIG_HOME` would work, but macOS
`ProjectDirs` ignores environment variables, so tests cannot be made
hermetic from the outside on the primary dev platform (repo work happens on
macOS per `.github`/dev setup).

## Suggested resolution

Add an ntropy-level override consulted before `ProjectDirs`, e.g.
`NTROPY_CONFIG_DIR` (directory) or `NTROPY_GLOBAL_CONFIG` (file path), in
`global::config_path()`. Then:

- set it to a per-test temp dir in the `ntropy()` helper in `tests/cli.rs`,
- pin both `Default vault: (not set)` and a set default in `info` snapshots
  (dropping the blanket redaction),
- add a contract test for `init --set-default` writing the config at the
  overridden location.

Document the variable alongside `NTROPY_VAULT` (ADR 0016 territory).

## Acceptance

- `cargo test` passes with a deliberately corrupt config at the real OS
  location (verifiable manually), because tests no longer read it.
- An `init --set-default` contract test exists.
