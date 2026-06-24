# Configuration

The concrete v1 config schema. Model and rationale are in
[ADR 0016](../adr/0016-configuration-format-location-and-vault-resolution.md);
view semantics in
[vault-layout-and-views.md](vault-layout-and-views.md).

Config is TOML, in two tiers.

## Global config

Location: the OS-native config directory (via `directories`): `~/.config/ntropy/config.toml`
on Linux, `~/Library/Application Support/ntropy/config.toml` on macOS.

v1 holds a single field:

```toml
# Default vault, used when no --vault flag, $NTROPY_VAULT, or cwd walk-up
# resolves one.
default_vault = "/Users/jakob/notes"
```

The editor is taken from `$VISUAL`/`$EDITOR` (ADR 0015), not config. There is
no color setting (v1 is plain, ADR 0024).

## Per-vault config

Location: `<vault>/.ntropy/config.toml`. Holds the view definitions, so they
travel with the vault.

```toml
# Each view is a top-level directory in the vault whose name is the table's
# `name` and whose tree groups notes by `field`. Grouping values are always
# normalized (ADR 0009 / ADR 0023).
[[view]]
name = "by-tag"
field = "tags"

[[view]]
name = "by-status"
field = "status"
```

`view list|add|edit|remove` (ADR 0018) read and write this file.

Templates are not in config; they live as files under `<vault>/.ntropy/templates/`
(`default.md` in v1, ADR 0017).

## Reserved names

Within a vault, `all-notes`, `.ntropy`, and any configured view `name` are
reserved (ADR 0007). A view `name` must not collide with `all-notes` or another
view.
