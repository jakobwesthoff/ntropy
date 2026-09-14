---
title: Encrypted vaults
tags: [docs/notes]
site:
  order: 6
---
A vault can store its notes encrypted at rest, so the service that syncs your notes cannot read them. This page covers what encryption protects, how unlocking works, the commands that convert a vault, and how scripts get in without a prompt.

```bash
ntropy init ~/private --encrypted     # asks for a passphrase
```

Everything else works as before. After a one-time `ntropy unlock` the key lives in your OS credential store, and `new`, `search`, `render` and the rest behave as in a plaintext vault. `ntropy lock` forgets it again. There is no timeout: unlocked means unlocked until you lock. If a command needs the key on a locked vault and you are at a terminal, it asks for the passphrase and stores the key as if you had run `unlock`.

## What this protects, and what it doesn't

Encryption defends the content of your notes against whoever stores or syncs the vault directory: Dropbox, iCloud, a git host, anyone who ends up with a copy. They see ciphertext. Each note is stored as `all-notes/<ulid>.age`; the slug is dropped from the filename because it would leak the title.

It does not hide metadata. Filenames still reveal each note's id and therefore its creation time, along with how many notes you have and how big each one is. `.ntropy/` stays plaintext in its entirety (config, templates, view definitions), which is what lets `new` work without a key. Nor does encryption reach backwards: if the vault was plaintext and synced before you encrypted it, those old revisions are still readable in your provider's version history, and cleaning that up is your job.

Local access to an unlocked session is outside the model too. Anyone using your logged-in session can read notes until you run `ntropy lock`.

## Converting a vault

```bash
ntropy vault encrypt      # convert an existing vault
ntropy vault decrypt      # and back again
ntropy vault rekey        # re-encrypt everything to a fresh key (same passphrase)
ntropy vault passphrase   # change the passphrase; notes are untouched
```

`encrypt`, `decrypt` and `rekey` rewrite the whole vault, so each asks first (`-y` skips) and each is safe to interrupt: every note is written in its new form and verified against the old one before anything is deleted, so a conversion killed halfway leaves both copies on disk. The next command refuses to touch the vault and tells you to finish with `--resume`. `vault encrypt` also removes the view trees, since views are disabled in an encrypted vault.

`vault passphrase` rewrites only the file holding the wrapped key; the notes stay as they are. `vault rekey` keeps the current passphrase unless `--new-passphrase-file` names a new one.

## Writing needs no key

The vault has one keypair, and only the public half is needed to encrypt. `ntropy new` therefore works on a locked vault: you can jot something down without unlocking anything. So does `ntropy write`, which is what makes an encrypted vault scriptable. Reading, searching and editing need the key, so `ntropy today` (which finds today's note by title) fails on a locked vault with a message naming `ntropy unlock`.

A plaintext `.md` note dropped into an encrypted vault's `all-notes/` by hand is skipped with a warning; `ntropy reconcile` encrypts it in place, and that works on a locked vault too.

## Scripts and headless machines

`--identity <path>` (`-i`) or `$NTROPY_IDENTITY` names an age identity file to use instead of the credential store, and `--passphrase-file <path>` supplies a passphrase from a file's first line rather than a prompt. Neither writes anything to your credential store; only a passphrase you typed leaves the vault unlocked afterwards. With `-n`, ntropy never prompts: a command that needs the key fails with a message naming `ntropy unlock` rather than blocking on a terminal that isn't there.

> [!NOTE]
> `--print`/`-p` reports the real path, which in an encrypted vault is the ciphertext file. That is fine for `stat` or `xargs rm`, not for reading. Use `search -P`/`--print-content` to get the note's text instead, which reads the same in either kind of vault. Editing by hand goes through `ntropy search`; `ntropy write` puts content back without an editor.

## What behaves differently

Two things change in an encrypted vault. [Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md) are disabled, because a `by-tag/` symlink tree would spell out your whole tag taxonomy in plaintext directory names inside the synced folder. `view add` is refused, while `view list` and `view remove` keep working so a vault encrypted after the fact can be cleaned up. And a rendered document or an exported site is plaintext by nature, so if you write one into the vault ntropy warns you that it will sync unencrypted.

## Under the hood

It is standard [age](https://age-encryption.org) throughout: every file ntropy writes is a plain age file. The vault's keypair is an X25519 pair. `.ntropy/identity.pub` holds the public key and `.ntropy/identity.age` the secret key, wrapped with your passphrase the same way `age -p` does it. With the stock `age` CLI and your passphrase you can recover a vault without ntropy at all.

> [!NOTE]
> Encryption is a default-on cargo feature. `cargo install ntropy --no-default-features` builds without the cryptography dependencies; such a build recognizes an encrypted vault and says it cannot open it.

Full details: [docs/design/encryption.md](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/encryption.md).
