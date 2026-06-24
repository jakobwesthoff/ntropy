# 16. Configuration format, location and vault resolution

Date: 2026-06-24

## Status

Accepted

## Context

Some configuration is global (default vault path, user preferences) and some is
per-vault (view definitions, templates, which reference that vault's fields).
ntropy also needs a rule for deciding which vault a given invocation operates
on.

## Decision

Config format is TOML.

Two tiers:

- Global config in the OS-native config directory, resolved with the
  `directories` crate (`ProjectDirs`): XDG on Linux,
  `~/Library/Application Support/ntropy` on macOS, Roaming `AppData` on
  Windows. Holds the default vault path and global preferences.
- Per-vault config under the vault's reserved `.ntropy/` directory. Holds view
  definitions and templates, so this configuration travels with the vault.

Vault resolution order: `--vault` flag, then `$NTROPY_VAULT`, then a git-style
walk up from the current directory to the nearest ancestor containing
`.ntropy/`, then the global config's default vault.

## Consequences

- View/template config travels with the vault; "my setup" stays global.
- Running ntropy inside a vault directory works without flags via cwd walk-up.
- macOS uses the native `~/Library/Application Support` path rather than
  `~/.config`, which some CLI users dislike.
- `.ntropy/` is confirmed as a reserved per-vault directory (ADR 0007).
