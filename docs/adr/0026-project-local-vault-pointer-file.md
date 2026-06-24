# 26. Project-local vault pointer file

Date: 2026-06-25

## Status

Accepted

## Context

ADR 0016 resolves a vault by walking up from the cwd to the nearest ancestor
containing `.ntropy/`. That only finds a vault you are physically inside. A
project may instead want to point at a vault that lives elsewhere (nested in
the project, or external) and have ntropy use it automatically from anywhere in
the project tree.

## Decision

The cwd walk-up also honors a marker file named `.ntropy-vault`. It contains a
single-line path to the vault: a relative path resolves against the marker
file's own directory, and absolute paths and `~` are also accepted.

During the walk, each directory is checked for both signals. The nearest
directory with either signal wins. If a single directory has both a `.ntropy/`
dir and a `.ntropy-vault` file, the marker wins, since it is an explicit
redirect.

The resolved target must be a vault (contain `.ntropy/`); if it is missing or
not a vault, resolution fails with an error.

The full resolution order is therefore: `--vault` > `$NTROPY_VAULT` > cwd
walk-up (`.ntropy-vault` marker or `.ntropy/` dir, nearest wins) > global
default vault.

## Consequences

- A project can carry a `.ntropy-vault` pointer so ntropy uses the intended
  vault from any subdirectory, enabling per-project vaults.
- `.ntropy-vault` is a new reserved filename ntropy looks for during walk-up.
- A broken pointer is a hard error rather than a silent fall-through to the
  global default, so misconfiguration is visible.
