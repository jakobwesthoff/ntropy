# Implement vault encryption

The full design is settled and lives in
[docs/design/encryption.md](../docs/design/encryption.md): opt-in per-vault
at-rest encryption against a sync/hosting-provider threat model, one age
X25519 keypair per vault (`.ntropy/identity.pub` plaintext recipient,
`.ntropy/identity.age` scrypt-passphrase-wrapped and armored), notes as
`all-notes/<ulid>.age`, OS-keychain-backed locking, secure-temp edit and
render round trips, views disabled, LSP supported via `NTROPY_VAULT_HINT`.
This todo is the entry point for building it; the design doc is normative,
this file carries only what is not in it.

## First step: write the ADR

No ADR records the decision yet. Write it in the ADR 0038 style: brief,
stating the decision and delegating the model to the design doc. Alternatives
that were considered and rejected during the design discussion, for the
rejected-alternatives section:

- **Granularity:** per-note opt-in encryption (kept the plain-text vault
  story but was rejected in favor of whole-vault); relying on filesystem
  encryption only (FileVault, encrypted volumes — does not cover the
  sync-provider threat model); no encryption in ntropy at all.
- **Format/library:** shelling out to an installed `age`/`rage` binary
  (subprocess per note in the scan path of every command, hard runtime
  dependency for core functionality); a custom format on RustCrypto
  primitives, including an Argon2id passphrase wrap (owns nonce/format/
  versioning responsibility and loses the stock-age escape hatch).
- **Key caching:** an ssh-agent-style daemon (extra process and socket
  protocol); prompting for the passphrase on every invocation; refusing
  locked reads until an explicit `unlock` (rejected for a TTY prompt that
  auto-stores instead).
- **Unlock TTL:** rejected; rationale is stated in the design doc's locking
  section.
- **Keychain keying:** by vault path (orphans the entry when the vault
  directory moves; recipient chosen instead).
- **Migration marker:** an existence-only marker file (rekey needs the
  target recipient stored anyway; `migration.toml` chosen).
- **History notice:** printing only when a `.git` directory is detected
  (no reliable marker exists for sync-synced vaults; unconditional chosen).
- **Render output inside an encrypted vault:** allowing silently with a doc
  caveat; refusing without `--force`. The stderr warning was chosen.
- **Views:** keeping symlink trees in place with a documented leak. Disabled
  instead; the relocation idea is
  [01ky7r9ean3b4bxs6fehpcg4ny-encrypted-vault-views.md](01ky7r9ean3b4bxs6fehpcg4ny-encrypted-vault-views.md).
- **CLI shape:** all six verbs top-level; all six grouped under one
  namespace; `crypt` or `key` as the namespace name. Chosen: `lock`/`unlock`
  top-level, the rest under `vault`, and no relocation of existing verbs
  (`reconcile`, `info`, `init` were considered for the `vault` namespace and
  left in place because moving released verbs is a breaking change).
- **LSP:** deferring encrypted-vault support to a later release was
  considered and rejected; it ships with the initial implementation.

## Validate at implementation time

- **`keyring` crate platform behavior:** the fallback chain (Secret Service
  → kernel keyring → prompt) on headless Linux, and the macOS behavior for
  unsigned binaries (keychain ACLs bind to the code signature, so a
  `cargo install` binary triggers an allow dialog once per binary).
- **`age` crate API:** the design doc names capabilities, not methods; the
  crate's API shifted between 0.9 and 0.11, so pin invocations against the
  current release.

## Deliberately left to implementation

- The exact `migration.toml` schema beyond its two stated fields (operation,
  target recipient).
- Module layout (library/binary split for the crypto, locking, and migration
  code) and the corresponding design-doc section.
- Whether the doc gains a performance statement (decided against for now).

## Design docs to update in the same unit of work

Per the documentation convention, these gain their encrypted-vault deltas
when the code lands, not before:

- `cli.md`: `lock`, `unlock`, the `vault` namespace, `--identity`/`-i`,
  `NTROPY_IDENTITY`, `init --encrypted`.
- `vault-layout-and-views.md`: the `<ulid>.age` layout, plaintext `.ntropy/`,
  views disabled.
- `language-server.md`: `NTROPY_VAULT_HINT`, `*.age` watching, keychain key
  access, locked-vault inertness.
- `rendering.md`: secure-temp staging, title-derived artifact names, the
  in-vault output warning.
- README as user-facing surface changes.
