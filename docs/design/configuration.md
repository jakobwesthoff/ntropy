# Configuration

The concrete v1 config schema. Model and rationale are in
[ADR 0016](../adr/0016-configuration-format-location-and-vault-resolution.md);
view semantics in
[vault-layout-and-views.md](vault-layout-and-views.md).

Config is TOML, in two tiers.

## Global config

Location: the OS-native config directory (via `directories`): `~/.config/ntropy/config.toml`
on Linux, `~/Library/Application Support/ntropy/config.toml` on macOS.

`$NTROPY_CONFIG_DIR` replaces that directory for reading and writing: when it
is set and not empty, the global config is `$NTROPY_CONFIG_DIR/config.toml`. An
empty value counts as unset, and a relative path is relative to the working
directory ([ADR 0061](../adr/0061-global-config-directory-override.md)).

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
# normalized (ADR 0009 / ADR 0023 / ADR 0077).
[[view]]
name = "by-tag"
field = "tags"

[[view]]
name = "by-status"
field = "status"
```

`view list|add|remove` (ADR 0018) read and write this file.

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
root = "tags/docs"                   # the sidebar starts at this tag page (its path form)

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

## Trust in a discovered vault

The walk-up and the pointer adopt a vault without asking, so a command run
inside a cloned repository, a synced folder or an extracted archive uses that
tree's vault: its configuration, templates, themes and notes. ntropy treats
all of it as data
([ADR 0070](../adr/0070-trust-model-for-discovered-vaults.md)):

- No configuration value, per-vault or global, selects an executable, a
  shell command, a hook, a pager or an argument of a process ntropy starts.
  Where the vault is and which note is worked on still shape the paths
  ntropy passes. The editor comes from `$VISUAL` and `$EDITOR` only
  (ADR 0066), and the `pdf` format runs `typst`, a name fixed in the engine
  and found on `$PATH` (ADR 0040).
- Vault content that is code runs only inside an interpreter that cannot
  start a process: a Typst theme inside the `typst` compiler, whose `--root`
  keeps the paths a document names inside the vault but which follows a
  symlink inside it, and a site theme's templates inside the embedded
  minijinja engine.
- View names are validated before they become paths, and a view directory
  behind a symlink is skipped (ADR 0064).

`ntropy info` names the rule that resolved the vault, and the pointer file
when one was followed. What stays accepted, such as a committed pointer that
sends ntropy to another existing vault, is listed in ADR 0070.

## Reserved names

Within a vault, `all-notes`, `.ntropy`, `.gitignore`, `README.md`, and the
directory of every configured view are reserved (ADR 0007). A view `name` must
not repeat another view's, and its first segment must not be one of the
reserved names; the full rules for view names, and what happens to a
configured view whose name breaks them, are in
[vault-layout-and-views.md](vault-layout-and-views.md) (ADR 0064).
`.ntropy-vault` is reserved as the pointer-file name during walk-up
(ADR 0026).
