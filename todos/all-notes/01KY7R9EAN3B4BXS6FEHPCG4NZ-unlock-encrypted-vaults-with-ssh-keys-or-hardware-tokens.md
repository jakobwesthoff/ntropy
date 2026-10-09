---
title: "Unlock encrypted vaults with SSH keys or hardware tokens"
kind: feature
component: crypto
horizon: someday
origin: idea
---
# Unlock encrypted vaults with SSH keys or hardware tokens

An encrypted vault's identity is wrapped by a passphrase (an age scrypt
recipient, see `docs/design/encryption.md`). The `age` crate also supports
encrypting to SSH public keys (`ssh-ed25519`, `ssh-rsa`) and to
`age-plugin-*` identities such as hardware keys (YubiKeys).

## Proposal

In ntropy's single-vault-keypair architecture this would only surface as an
alternative or additional way to wrap the one vault identity in
`.ntropy/identity.age`. A vault could unlock with an SSH key or a hardware
token instead of the passphrase, or alongside it. It would never be a
per-note recipient scheme.

## Revisit when

No trigger known; reconsider at the next sweep.
