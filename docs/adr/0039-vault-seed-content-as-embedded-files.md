# 39. Vault seed content as embedded files

Date: 2026-07-10

## Status

Accepted

Supplies the content for the layout of
[ADR 0007](0007-vault-directory-layout.md) and the templates of
[ADR 0017](0017-note-templates-with-placeholder-substitution.md).

## Context

The content `init` writes into a fresh vault lived as Rust string literals:
the default and `today` templates in `src/template.rs`, the vault `README.md`
in `src/ops/init.rs`. Markdown inside a Rust literal is not Markdown to any
tool that reads the repository, and the README's shell examples carried
backslash-escaped quotes to survive the literal.

## Decision

Seed content lives as real files under `src/vault/seed/`, embedded with
`include_str!` by the `vault::seed` module.

- The tree mirrors the vault it produces, except that the config directory is
  spelled `ntropy/` rather than `.ntropy/`. `layout::is_vault` keys purely on
  a `.ntropy/` directory existing, so a faithful dot-directory would make
  `src/vault/seed/` resolve as a vault to ntropy's own path lookup.
- `vault::seed` sits beside `vault::layout`: layout names a vault's
  well-known files, seed holds their initial contents. Both are consumed from
  above by `template` and `ops::init`.
- The seed files carry no MPL-2.0 header, the sole exception to the rule in
  `CLAUDE.md`. They are copied verbatim into user vaults. A unit test in
  `vault::seed` fails if a header appears in one.
- `ops::init` writes them by iterating a `SEEDED_FILES` manifest pairing a
  `Layout` accessor with its content constant, rather than one call per file.
- The per-vault `config.toml` is not seed content. It stays constructed from
  `PerVaultConfig`, which is also the type that reads it back.

## Consequences

- The seed files are editable, diffable, and lintable as Markdown, and the
  README's shell examples hold plain quotes.
- Adding a seeded file means adding it under `src/vault/seed/` and listing it
  in `SEEDED_FILES`; nothing else in `init` changes.
- `Cargo.toml` declares no `include`/`exclude`, so the tree ships in the
  packaged crate.
- `ntropy::template::DEFAULT_TEMPLATE` and `TODAY_TEMPLATE` moved to
  `ntropy::vault::seed`. The library is internal to the CLI, so this is not a
  versioned interface change.
