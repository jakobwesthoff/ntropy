# 41. Opt-in vault encryption with age

Date: 2026-08-06

## Status

Accepted

Interprets the headless-library rule of
[ADR 0013](0013-library-binary-split-with-thiserror-and-anyhow.md) the same
way [ADR 0038](0038-pluggable-rendering-engine-with-pandoc-and-typst.md) does,
by injecting the effect rather than performing it. Note identity stays as
[ADR 0004](0004-note-identity-and-filename-strategy.md) fixed it and the link form
as [ADR 0028](0028-note-to-note-links-as-standard-markdown-links.md) fixed it;
prompting keys off the controlling terminal per
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md); testing
follows [ADR 0021](0021-testing-strategy-with-insta-across-all-layers.md).
`docs/design/encryption.md` carries the full model.

## Context

A vault is a directory of Markdown files, and the ordinary way to keep one
across machines is a sync provider or a git host. That party sees every note.
Filesystem encryption does not help: it protects the disk, not the copy the
provider holds.

## Decision

### One age keypair per vault, chosen at creation

`ntropy init --encrypted` generates an age X25519 keypair.
`.ntropy/identity.pub` holds the recipient in plaintext and its presence is
what marks a vault encrypted, so detection is stateless. `.ntropy/identity.age`
holds the identity wrapped with age's own scrypt passphrase recipient,
ASCII-armored.

Notes live as `all-notes/<ulid>.age`, one standard age file each. The slug
leaves the filename, since it would spell out the title; it is derived from the
decrypted title wherever it is needed. `.ntropy/` stays plaintext in its
entirety.

No ntropy-specific cryptography exists. Every artifact is a standard age file,
so the stock `age` CLI and the passphrase are enough to recover a vault without
ntropy.

### Writing needs no key

Splitting the recipient from the identity makes note creation work on a locked
vault while reading, searching, editing and migrating require the identity.
Keeping `.ntropy/` plaintext is what makes that real: `new` must read a
template before it can encrypt anything.

### One seam for note I/O

A `NoteCipher` abstraction mediates every note read and write, with a
passthrough implementation for plaintext vaults and an age implementation for
encrypted ones. Scanning, reconcile and note creation share one code path
rather than branching per call site. Policy differences that are not about
reading or writing a file — materialized views are disabled, filename
realignment is inapplicable — do branch on the vault's shape.

### Keys are acquired in the library, prompted from the binary

The retrieval chain is an explicit identity file, then the OS keychain, then a
passphrase file, then an interactive prompt. It lives in the library, shaped
like `config::global`: all logic in functions taking an explicit store, thin
wrappers resolving the real keychain. The prompt is a trait the binary
implements over the controlling terminal, so the operations layer stays
headless.

### Migrations are crash-safe by being additive

`vault encrypt`, `vault decrypt` and `vault rekey` record a marker in
`.ntropy/migration.toml` that makes every ordinary command refuse while it
exists. Each target file is produced beside its source, then verified by
reading it back and comparing plaintext, and only then are the sources deleted.
`--resume` re-verifies by comparison rather than by existence, so a target that
was written but not durable is produced again.

### Encryption is a build-time option

The `encryption` cargo feature, enabled by default, gates the cryptography and
credential-store dependencies. The command surface is compiled unconditionally
and reports the missing support at runtime, so a stripped build and a full one
present the same interface.

### Testing

Encrypted fixture vaults are built at test setup from a fixed test keypair
rather than committed, since age ciphertext is randomized and never
snapshot-stable. Tests assert round trips and plaintext-level outcomes. Key
retrieval sits behind the store trait, and the CLI contract tests pass an
explicit identity and passphrase file, so no test reaches a real keychain or a
terminal.

## Rejected alternatives

- **Granularity:** per-note opt-in encryption, which kept the plain-text vault
  story; relying on filesystem encryption alone, which does not cover the
  sync-provider threat model; no encryption in ntropy at all.
- **Format and library:** shelling out to an installed `age`/`rage` binary, a
  subprocess per note in the scan path of every command and a hard runtime
  dependency for core functionality; a custom format over RustCrypto
  primitives with an Argon2id passphrase wrap, which owns nonce, format and
  versioning responsibility and loses the stock-age escape hatch.
- **Key caching:** an ssh-agent-style daemon, an extra process and socket
  protocol; prompting for the passphrase on every invocation; refusing locked
  reads until an explicit `unlock`, in favour of a terminal prompt that stores
  what it obtains.
- **Unlock TTL:** rejected; the threat model excludes local access to an
  unlocked session, so expiry would defend against nothing inside it.
- **Keychain keying:** by vault path, which orphans the entry when the vault
  directory moves; the recipient is used instead.
- **Migration marker:** an existence-only marker file; `rekey` needs the target
  recipient recorded regardless.
- **History notice:** printing only when a `.git` directory is detected. No
  reliable marker distinguishes a synced vault, so the notice is unconditional.
- **Render output inside an encrypted vault:** allowing it silently with a
  documented caveat; refusing without `--force`. A stderr warning was chosen.
- **Views:** keeping the symlink trees with a documented leak. They are
  disabled instead; relocating them outside the vault is deferred.
- **CLI shape:** all six verbs at the top level; all six under one namespace;
  `crypt` or `key` as the namespace name. `lock` and `unlock` are top level and
  the rest sit under `vault`. Released verbs were left where they are, since
  moving them would break scripts.
- **Language server:** deferring encrypted-vault support to a later release.
- **Headless passphrase supply:** an `NTROPY_PASSPHRASE` environment variable,
  readable through `ps` and `/proc`; a command exporting the raw identity to a
  file. A passphrase file was chosen for both generation and acquisition.
- **Feature gating the command surface out:** it makes `--help` differ between
  builds, so every help snapshot would need one variant per configuration.
- **Testing the editor round trip through a pseudo-terminal:** ADR 0021 already
  validates terminal-attached code manually and unit-tests the logic behind it;
  the round trip follows the picker's precedent.

## Consequences

- New dependency trees join the runtime, none of them linking a C library.
  `age` is pure Rust throughout. Credential stores are named individually
  against `keyring-core` — the macOS Keychain, the pure-Rust zbus Secret
  Service client, and the Linux kernel keyring — rather than through the
  `keyring` facade, which binds one store per platform with no fallback and
  whose `cli` feature would build a vendored libdbus from C source.
  Distribution is unchanged.
- Behaviour becomes vault-shape-dependent in ways that are not about
  ciphertext: materialized views do not exist in an encrypted vault, filename
  realignment has nothing to realign, and editing round-trips through a
  temporary file outside the vault.
- The library gains its second injected-capability seam after `RenderContext`.
- Metadata is not protected. Filenames reveal each note's ULID and therefore
  its creation time, along with the note count and file sizes, and `.ntropy/`
  is readable in full.
