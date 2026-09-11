---
title: Finding the vault
tags: [docs/notes]
site:
  order: 4
---
Every ntropy command operates on exactly one vault. This page describes how that vault is chosen, how a project can point at a vault kept elsewhere, and how to check which vault a command would use.

## Resolution order

ntropy resolves the vault in this order and stops at the first rule that matches:

1. `--vault <path>`
2. `$NTROPY_VAULT`
3. A walk up from the current directory to the nearest ancestor holding a `.ntropy-vault` pointer file or a `.ntropy/` directory. The nearest ancestor wins. A pointer beats a `.ntropy/` in the same directory, since it is an explicit redirect.
4. The global default vault, set with `ntropy init --set-default`.

A path given by `--vault` or `$NTROPY_VAULT` has to be a vault already, meaning a directory holding `.ntropy/`. If it is not, the command fails instead of trying the next rule. When no rule matches, ntropy fails with a message listing the four ways to name a vault.

## Project-local pointers

Step 3 is the one worth setting up. A `.ntropy-vault` file is a single line naming a vault elsewhere: a path relative to the file's own directory, an absolute path, or one starting with `~`.

```
# project/.ntropy-vault
../notes
```

Drop one at the root of a project and ntropy uses that vault from anywhere inside the project, so `ntropy new` and `ntropy search` work for project notes the same way they work everywhere else. A pointer whose target is not a vault, or an empty pointer file, is a hard error, never a silent fall-through to the default.

## Checking which vault resolved

`ntropy info` prints the active vault together with the rule that resolved it (the `--vault` flag, `$NTROPY_VAULT`, a pointer file, the current directory, or the global default), then the configured default vault and statistics for the active one. `ntropy info -p` (`--print`) prints the vault path alone, for shell use.
