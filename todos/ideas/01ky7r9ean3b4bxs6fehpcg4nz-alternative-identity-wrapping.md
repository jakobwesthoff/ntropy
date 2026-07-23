# Alternative identity wrapping (SSH keys, hardware keys)

Future idea, not scheduled.

An encrypted vault's identity is wrapped by a passphrase (age scrypt
recipient, [docs/design/encryption.md](../../docs/design/encryption.md)). The
`age` crate also supports encrypting to SSH public keys (`ssh-ed25519`,
`ssh-rsa`) and to `age-plugin-*` identities (hardware keys such as YubiKeys).

In ntropy's single-vault-keypair architecture this would only ever surface as
an alternative or additional way to wrap the one vault identity in
`.ntropy/identity.age` — for example unlocking with an SSH key or hardware
token instead of, or alongside, the passphrase. It would never be a per-note
recipient scheme.
