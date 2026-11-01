---
title: Encrypted vaults
tags: [docs/notes]
site:
  order: 3
---
A vault can store its notes encrypted at rest, so the service that syncs
your notes cannot read them. Create one with `--encrypted`, or convert
an existing vault:

```bash
ntropy init ~/private --encrypted     # asks for a passphrase
ntropy vault encrypt                  # converts the vault you are in
```

Everything else works as before. After a one-time `ntropy unlock` the key
lives in your OS credential store, and `new`, `search`, `render` and the
rest behave as in a plaintext vault. `ntropy lock` forgets the key again.
There is no timeout: unlocked means unlocked until you lock. If a command
needs the key on a locked vault and you are at a terminal, it asks for
the passphrase and stores the key as if you had run `unlock`.

## Writing needs no key

The vault has one keypair, and only its public half is needed to
encrypt. `ntropy new` therefore works on a locked vault: you can jot
something down without unlocking anything. So does `ntropy write`, which
is what makes an encrypted vault scriptable. Reading, searching and
editing need the key. `ntropy today` finds today's note by its title, so
it fails on a locked vault, with a message naming `ntropy unlock`.

A plaintext `.md` note dropped into an encrypted vault's `all-notes/` by
hand is skipped with a warning. If it is a well-formed note, named
`<ulid>-<slug>.md` and with a `title`, `ntropy reconcile` encrypts it in
place, and that works on a locked vault too. Any other file stays as it
is. If the note's ULID already belongs to an encrypted note, as with a
copy, it first gets a ULID of its own with the same creation time, so
neither note is overwritten.

## What behaves differently

[Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md)
are disabled, because a `by-tag/` symlink tree would spell out your whole
tag taxonomy in plaintext directory names inside the synced folder.
`view add` is refused. `view list` and `view remove` keep working, so a
vault encrypted after the fact can be cleaned up.

A rendered document or an exported site is plaintext by nature. If you
write one into the vault, ntropy warns that it will sync unencrypted.

## Scripts and headless machines

`--identity <path>` (`-i`) or `$NTROPY_IDENTITY` names an age identity
file to use instead of the credential store. `--passphrase-file <path>`
supplies a passphrase from a file's first line instead of a prompt.
Neither writes anything to your credential store; only a passphrase you
typed leaves the vault unlocked afterwards. With `-n`, ntropy never
prompts: a command that needs the key fails with a message naming
`ntropy unlock` instead of waiting on a terminal that is not there.

> [!NOTE]
> `--print`/`-p` reports the real path, which in an encrypted vault is
> the ciphertext file. That is fine for `stat` or `xargs rm`, not for
> reading. `search -P`/`--print-content` gives the note's text instead,
> and reads the same in either kind of vault. Editing by hand goes
> through `ntropy search`; `ntropy write` puts content back without an
> editor.

## Converting a vault

```bash
ntropy vault encrypt      # convert an existing vault
ntropy vault decrypt      # and back again
ntropy vault rekey        # re-encrypt everything to a fresh key (same passphrase)
ntropy vault passphrase   # change the passphrase; notes are untouched
```

- `encrypt`, `decrypt` and `rekey` rewrite the whole vault. Each asks
  first; `-y` skips the question, and under `-n` it is required.
- Each is safe to interrupt. Every note is written in its new form and
  verified against the old one before anything is deleted, so an
  `encrypt` or `decrypt` killed halfway leaves both copies on disk.
- A `rekey` writes its new key into `.ntropy/` before it replaces the
  first note, and keeps the old key until it has replaced the last.
  Wherever it is killed, every note opens with a key on disk.
- After an interrupted run, the next command refuses to touch the vault
  and tells you to finish with `--resume`. For a `rekey`, `--resume`
  needs the passphrase the new key was given, which is the one in
  `--new-passphrase-file` if the interrupted run had one.
- `vault encrypt` removes the view trees. For a nested view such as
  `area/work`, it also removes the parent directories that are empty
  once the tree is gone (`area`); a parent that holds anything else
  stays.
- `vault encrypt` refuses to start while a configured view has an
  invalid name or a symlink in place of its directory, because it could
  not remove that view's tree safely. The message says what to remove
  first.
- `encrypt`, `decrypt` and `rekey` refuse to start while two note files
  share a ULID, such as a note copied by hand, or while a file already
  sits where a converted note would be written, since either would
  overwrite a note. Run `ntropy reconcile` first, or move the named files
  away.

`vault passphrase` rewrites only the file holding the wrapped key; the
notes stay as they are. `vault rekey` keeps the current passphrase
unless `--new-passphrase-file` names a new one.

## What this protects, and what it doesn't

Encryption defends the content of your notes against whoever stores or
syncs the vault directory: Dropbox, iCloud, a git host, anyone who ends
up with a copy. They see ciphertext. Each note is stored as
`all-notes/<ulid>.age`; the slug is dropped from the file name because
it would leak the title.

It does not hide metadata. File names still reveal each note's id and
therefore its creation time, along with how many notes you have and how
big each one is. `.ntropy/` stays plaintext in its entirety (config,
templates, view definitions), which is what lets `new` work without a
key. Nor does encryption reach backwards: if the vault was plaintext and
synced before you encrypted it, those old revisions are still readable
in your provider's version history, and cleaning that up is your job.

Local access to an unlocked session is outside the model too. Anyone
using your logged-in session can read notes until you run
`ntropy lock`.

## Under the hood

It is standard [age](https://age-encryption.org) throughout: every file
ntropy writes is a plain age file. The vault's keypair is an X25519 pair.
`.ntropy/identity.pub` holds the public key and `.ntropy/identity.age`
the secret key, wrapped with your passphrase the same way `age -p` does
it. With the stock `age` CLI and your passphrase you can recover a vault
without ntropy at all.

> [!NOTE]
> Encryption is a default-on cargo feature.
> `cargo install ntropy --no-default-features` builds without the
> cryptography dependencies. Such a build recognizes an encrypted vault
> and says it was compiled without encryption support.

Full details:
[docs/design/encryption.md](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/encryption.md).
