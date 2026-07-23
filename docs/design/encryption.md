# Encryption

Vault encryption at rest: how an encrypted vault stores, unlocks, edits, and
migrates notes. The command surface it extends is in [cli.md](cli.md), the
vault layout in [vault-layout-and-views.md](vault-layout-and-views.md), the
language server in [language-server.md](language-server.md), rendering in
[rendering.md](rendering.md).

Encryption is opt-in per vault, chosen at creation: `ntropy init --encrypted`
prompts for a passphrase and produces a vault whose notes are ciphertext on
disk. Daily use is transparent: after a one-time unlock, every command works
as in a plaintext vault.

## Threat model

An encrypted vault defends the *content* of notes against whoever stores or
syncs the vault directory: a cloud sync provider (Dropbox, iCloud), a git
hosting service, or anyone who obtains a copy of the synced data. Those
parties see only ciphertext.

Deliberately outside the model:

- **Local access to an unlocked session.** Once unlocked, the key sits in the
  OS keychain until an explicit `ntropy lock`; anyone using the logged-in
  session can read notes. There is no unlock timeout.
- **Metadata.** Filenames reveal each note's ULID (and thus its creation
  time), the note count, and file sizes. `.ntropy/` contents remain plaintext
  (see below).
- **History predating encryption.** Encrypting an existing vault does not
  reach into git history or a sync provider's version history; plaintext
  revisions recorded before the migration stay readable there.

## Key model

Each encrypted vault has exactly one [age](https://age-encryption.org)
X25519 keypair, generated at vault creation:

- `.ntropy/identity.pub` holds the recipient (the `age1...` public key) as
  one plaintext line. Its presence is what marks a vault as encrypted, so
  detection stays stateless.
- `.ntropy/identity.age` holds the identity (the `AGE-SECRET-KEY-1...`
  secret), encrypted with age's scrypt passphrase recipient and stored
  ASCII-armored so the file is text and survives text-mode transports.

The split gives an asymmetric property that shapes the whole design: *writing
needs no passphrase*. Any command that only creates notes encrypts them
against the public recipient and works on a locked vault. Reading, searching,
editing, and migrating need the identity.

The passphrase wrap is the age format's own scrypt recipient, the same
construction as `age -p`: the file key is wrapped with a key derived from the
passphrase via scrypt over a random salt. No ntropy-specific cryptography
exists anywhere in the design; every artifact is a standard age file. That
keeps the escape hatch open: with the stock `age` CLI and the passphrase, a
user can decrypt `.ntropy/identity.age` and then any note, without ntropy.

## On-disk layout

Notes in an encrypted vault live as `all-notes/<ulid>.age`: each note is an
individual age file encrypted to the vault recipient. The filename carries
only the ULID, which remains the canonical identity exactly as in a plaintext
vault; the slug component does not exist, since it would leak the title. The
title lives where it always lives, in the (now encrypted) frontmatter, and
anything derived from the slug derives it from the decrypted title instead.
The slug-realignment and rename machinery of plaintext vaults is inapplicable:
there is nothing in the filename to realign.

`.ntropy/` stays plaintext in its entirety: config, templates, and view
definitions. This is what preserves key-free note creation, because `new` and
`today` must read a template before they can encrypt anything. The leak is
template boilerplate and configuration structure, not note content.

Seed content is encrypted during `init --encrypted` like any other note.

A plaintext `.md` file appearing inside an encrypted vault's `all-notes/`
(for example dropped in by hand) is skipped by the scanner with a warning,
consistent with the scanner's robustness posture. `reconcile` adopts such a
file: it encrypts it in place, giving hand-added files a sanctioned path into
the vault.

## Locking model

A vault is *unlocked* when its identity is retrievable without user
interaction, and *locked* otherwise.

- `ntropy unlock` prompts for the passphrase, decrypts the identity, and
  stores it in the OS keychain. `ntropy lock` removes it. There is no
  timeout: unlocked means unlocked until `lock`.
- Retrieval tries, in order: the OS keychain (macOS Keychain; Secret Service
  on Linux), the Linux kernel keyring where no Secret Service daemon is
  available, and finally an interactive passphrase prompt.
- When a command needs the identity on a locked vault and a controlling
  terminal is present, it prompts for the passphrase and stores the result
  as if `unlock` had run, so the explicit command is mostly a formality.
  Without a controlling terminal the command fails with an error naming
  `ntropy unlock`.
- For scripts and headless environments, `--identity <path>` / `-i` or the
  `NTROPY_IDENTITY` environment variable names a plain age identity file to
  use instead of the keychain. The file is the native `age-keygen` format,
  so externally managed identities work unmodified.

What the keychain stores is the bare `AGE-SECRET-KEY-1...` string, under the
service name `ntropy` with the vault's `age1...` recipient as the account.
Keying by recipient rather than vault path identifies the keypair itself, so
a moved vault directory finds its entry unchanged; the lookup reads
`identity.pub` first in any case, since that is how a vault is recognized as
encrypted.

The absence of an unlock timeout is by design, not an omission: the threat
model excludes local access to the logged-in session, so expiry machinery
would defend against nothing inside the model.

## Command surface

Two daily verbs join the top level, and the rare, vault-rewriting operations
group under a `vault` namespace:

    ntropy lock
    ntropy unlock
    ntropy vault encrypt
    ntropy vault decrypt
    ntropy vault rekey
    ntropy vault passphrase

- `lock` / `unlock` manage the keychain entry as described above.
- `vault passphrase` changes the passphrase: it re-wraps
  `.ntropy/identity.age` under the new passphrase. One file changes; notes
  are untouched.
- `vault encrypt` converts a plaintext vault in place; `vault decrypt` is
  the inverse and requires the identity.
- `vault rekey` generates a fresh keypair and re-encrypts every note to it,
  for a suspected identity compromise. It requires the old identity.

### Migration crash safety

`encrypt`, `decrypt`, and `rekey` rewrite the whole vault and share one
pattern, built on the fact that the conversion is additive until its final
step:

1. `.ntropy/migration.toml` records the operation in progress
   (`encrypt`, `decrypt`, or `rekey`) and the target recipient. While it
   exists, every normal command refuses with an error naming the resume
   command, so a half-converted vault is never silently scanned or synced
   further.
2. Every target file is produced as a sibling of its source, written via
   write-temp-rename, leaving the sources untouched.
3. Each produced file is verified by decrypting it and comparing against its
   source. Only when every note verifies does the point of no return arrive:
   the sources are deleted (for `encrypt`, the view trees as well, since
   views are disabled in encrypted vaults) and the marker is removed.

A crash anywhere before the final deletion leaves both forms on disk, and
`--resume` is idempotent: produce whatever lacks a verified target, then
finish the deletion.

The `vault encrypt` completion report always states that plaintext revisions
previously recorded in git or a sync provider's version history remain
readable there; cleaning that history is the user's job. The line is
unconditional because no reliable marker distinguishes a synced vault from a
local-only one, and a vault that never synced loses nothing by hearing it.

## Scanning

Scanning remains stateless: every command re-reads `all-notes/`, and in an
encrypted vault that means decrypting every note in memory on each scan.
Nothing decrypted is written anywhere; plaintext exists only in process
memory. The identity comes from the locking model above, so the passphrase
is needed at most once, not per command.

## Editing

An editor needs a real plaintext file, so the edit round-trip decrypts to a
temporary file outside the vault, launches the editor on it, and re-encrypts
on exit:

- The temp file is created with `0600` permissions in the runtime directory:
  `$XDG_RUNTIME_DIR` where set (on Linux this is tmpfs, so plaintext never
  reaches a disk), otherwise the system temp directory as a best effort. On
  macOS there is no tmpfs equivalent; the gap between "temp file on disk"
  and "plaintext never on disk" is covered by FileVault, and documented as
  such.
- On editor exit the buffer is encrypted back into `all-notes/<ulid>.age`
  via write-temp-rename, then the temp file is overwritten and deleted.
- Because the temp file lives outside the vault directory, sync providers
  never see it regardless of platform.
- Editors leave their own artifacts (swap files, undo history, backup
  copies) wherever the user configured them. That is outside ntropy's
  control and is a documented residual risk.

`new` and `today` compose the same round-trip with creation: the note is
materialized from the template into the temp file, edited, and encrypted on
exit. No step needs the identity, so creation works on a locked vault.

## Rendering

Render staging uses the same secure temporary location as editing; engines
never stage plaintext inside the vault. Two encrypted-vault specifics:

- The default artifact name derives from the note's decrypted title, since
  there is no filename slug to take it from. The result is the same name a
  plaintext vault would produce.
- A render artifact is plaintext by nature. When the resolved output path
  lies inside an encrypted vault, the command proceeds but prints a one-line
  warning to stderr that the artifact is unencrypted and will sync as such.
  The common accidental case is a shell whose working directory happens to
  be the vault root, where the default `./<name>.pdf` would land the
  plaintext inside the synced directory.

## Views

Materialized views are disabled in encrypted vaults: a symlink tree like
`by-tag/` would spell out the tag taxonomy in plaintext names inside the
synced directory. View definitions in `.ntropy/` are inert there, and
`vault encrypt` removes existing view trees at the point of no return.

## Language server

The language server supports encrypted vaults with the same feature set as
plaintext vaults, with three adaptations:

- **Vault resolution.** The editor opens the decrypted temp file, whose path
  resolves to no vault. ntropy sets `NTROPY_VAULT_HINT=<vault-root>` when it
  launches the editor; the language server, spawned by that editor, inherits
  it and uses it as the fallback resolution for documents that resolve to no
  vault. An editor session not started by ntropy has no hint, but in an
  encrypted vault there is no on-disk plaintext to open directly, so that
  path does not arise.
- **Key access.** The server builds its scan cache by decrypting in memory,
  taking the identity from the same retrieval chain as every command. A
  language server has no controlling terminal, so it never prompts: on a
  locked vault it stays inert until an unlock happens elsewhere.
- **Watching.** The file watch covers `*.age` alongside `*.md`, so ciphertext
  changes trigger the usual full rescan.

## Testing

Age encryption is randomized (fresh file keys and ephemeral shares per file),
so ciphertext bytes are never snapshot-stable. Tests therefore snapshot
plaintext-level outcomes and assert round trips, in the established `insta`
style:

- **Fixture vaults are built, not committed.** Encrypted test vaults are
  constructed at test setup by encrypting the existing plaintext fixtures
  with a fixed test keypair and passphrase that live in the test tree.
  Fixtures stay readable and reviewable; no ciphertext is checked in.
- **Key retrieval sits behind a seam.** Commands take the identity through
  an abstraction the tests satisfy with an in-memory store; the standard
  suite never touches a real OS keychain or prompts for a passphrase.
- **The scripting path doubles as the CLI test seam.** Contract tests pass
  `--identity` with the test identity file, exercising the exact code path
  headless users run while keeping tests free of keychain state.
- **Migration tests interrupt on purpose.** Crash-safety tests inject a
  failure between the produce, verify, and delete phases, snapshot the
  resulting directory state (marker present, both file forms on disk), and
  assert that `--resume` converges to the same result as an uninterrupted
  run.

## Implementation

Both cryptography and keychain access are pure-Rust dependencies; no linked
C libraries, distribution unchanged:

- The `age` crate (the library under the `rage` CLI) provides X25519
  recipients and identities, the scrypt passphrase recipient, streaming
  encryption and decryption, and ASCII armor. Everything ntropy writes is a
  standard age file readable by stock tooling.
- The `keyring` crate abstracts the macOS Keychain and the Linux Secret
  Service and kernel keyring behind one entry API.
