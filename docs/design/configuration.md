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

`default_vault` is written only by `ntropy init --set-default` (or by editing
the file by hand). Without `--set-default`, `init` never touches the global
config. The editor is taken from `$VISUAL`/`$EDITOR` (ADR 0015), not config.
There is no color setting (v1 is plain, ADR 0024).

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

The same file holds an optional `[render]` table for the document formats
([rendering.md](rendering.md), "Configuration") and an optional `[site]`
table for the `html` format and the site export
([site-export.md](site-export.md), "Configuration"):

```toml
[site]
theme = "corporate"                  # .ntropy/themes/site/corporate/
index = "01ARZ3NDEKTSV4RRFFQ69G5FAV" # the note that becomes the front page
title = "Team Docs"                  # defaults to the vault directory name
lang = "en"                          # the html lang attribute, default en
related = false                      # no related notes under the pages
root = "tags/docs"                   # the sidebar starts at this tag page

[[site.nav]]                         # a hand-assembled sidebar section
label = "Getting Started"
items = [
  { note = "01ARZ3NDEKTSV4RRFFQ69G5FAV" },
  { label = "Gadgets", tag = "docs/start/gadgets" },
]

[site.vars]                          # free-form, for the theme's templates
github = "https://github.com/acme/docs"
```

Every key of both tables is optional, and an entirely default table is
omitted when ntropy writes the file.

Templates are not in config; they live as files under `<vault>/.ntropy/templates/`
(`default.md` in v1, ADR 0017).

## Project-local vault pointer

A directory anywhere above the cwd may carry a `.ntropy-vault` marker file
(ADR 0026) whose single line is a path to the vault — relative to the marker's
own directory, or absolute, or `~`-prefixed. The cwd walk-up honors it, so a
project can point at a vault (nested or external) and ntropy uses it from any
subdirectory. The pointer wins over a `.ntropy/` dir in the same directory; a
broken pointer is a hard error.

```text
# project/.ntropy-vault
./notes
```

## Reserved names

Within a vault, `all-notes`, `.ntropy`, and any configured view `name` are
reserved (ADR 0007). A view `name` must not collide with `all-notes` or another
view. `.ntropy-vault` is reserved as the pointer-file name during walk-up
(ADR 0026).
