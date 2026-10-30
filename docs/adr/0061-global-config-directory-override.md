# 61. Global config directory override

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0016](0016-configuration-format-location-and-vault-resolution.md),
whose global config always lived in the OS-native config directory.

## Context

Nearly every command reads the global config while it resolves the vault,
and `init --set-default` writes it. With the location fixed to the OS-native
config directory, a process that must not see the user's config, such as the
CLI contract tests, had no ntropy-level way to point it elsewhere.

## Decision

`$NTROPY_CONFIG_DIR`, when set and not empty, names the directory that holds
the global `config.toml`, for reading and for writing, in place of the
OS-native config directory. An empty value counts as unset, so a stray
`NTROPY_CONFIG_DIR=` cannot put a `config.toml` into whatever directory a
command runs in. A relative path is relative to the working directory.

## Consequences

- The CLI contract tests can keep the host's own global config out of their
  results, and `init --set-default` has a contract test.
