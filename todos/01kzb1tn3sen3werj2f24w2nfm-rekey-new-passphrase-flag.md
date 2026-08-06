# `vault rekey` handles its passphrase inconsistently

`vault rekey` needs a passphrase twice: to open the old identity, and to wrap
the new one it generates. It currently gets both from the same place, and the
two entry paths disagree about what that means:

- **Headless** (`--passphrase-file X`) uses `X` for both, so the vault keeps its
  passphrase and only the key changes.
- **Interactive** prompts for a *new* passphrase and confirms it, so the
  passphrase changes too.

Each is defensible alone. Together they mean the same command does two
different things depending on whether a terminal is attached, and there is no
way to change the passphrase during a headless rekey.

## Decided direction

Add `--new-passphrase-file <PATH>` to `vault rekey`, falling back to the global
`--passphrase-file` when absent. That reuses the flag `vault passphrase`
already has rather than inventing a second spelling, and keeps today's
behaviour as the default.

Rejected: making `rekey` never touch the passphrase and always re-wrap the new
key under the existing one. Conceptually cleaner — `rekey` changes the key,
`vault passphrase` changes the passphrase — but unworkable when the identity
came from the credential store and no passphrase is in hand to re-wrap with.

Interactive behaviour should follow: prompt only when no file was named.

Found while smoke-testing the finished feature; see
[docs/design/encryption.md](../docs/design/encryption.md) for the key model.
