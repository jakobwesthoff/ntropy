---
title: Finding the vault
tags: [docs/notes]
site:
  order: 2
---
Every ntropy command works on exactly one vault. Run inside a vault, a
command uses that vault, unless `--vault` or `$NTROPY_VAULT` names
another. This page shows how a project points at a vault kept
elsewhere, how to check which vault a command uses, and the full order
of the rules.

## A vault for a project

A `.ntropy-vault` file is a single line naming a vault elsewhere: a path
relative to the file's own directory, an absolute path, or one starting
with `~`.

```
# project/.ntropy-vault
../notes
```

Drop one at the root of a project, and ntropy uses that vault anywhere
inside the project. `ntropy new` and `ntropy search` then work for the
project's notes the way they work everywhere else. A pointer whose
target is not a vault, or an empty pointer file, is an error, never a
silent fall-through to the default vault.

## Check which vault a command uses

`ntropy info` prints the active vault and the rule that resolved it: the
`--vault` flag, `$NTROPY_VAULT`, a pointer file, the current directory,
or the global default. Then it prints the configured default vault and
statistics for the active one.

## Resolution order

ntropy tries these rules in order and stops at the first that matches:

1. `--vault <path>`
2. `$NTROPY_VAULT`
3. A walk up from the current directory to the nearest directory holding
   a `.ntropy-vault` pointer file or a `.ntropy/` directory. The nearest
   one wins. A pointer beats a `.ntropy/` in the same directory, since it
   is an explicit redirect.
4. The global default vault, set with `ntropy init --set-default`. It is
   kept in the global config, which `$NTROPY_CONFIG_DIR` can move; see
   [Configuration](01M28Q32E6BHWB0H27MM57KY90-configuration.md).

A path given by `--vault` or `$NTROPY_VAULT` has to be a vault already,
a directory holding `.ntropy/`. If it is not, the command fails instead
of trying the next rule. When no rule matches, ntropy fails with a
message listing the four ways to name a vault.
