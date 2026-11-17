---
title: "Decide whether ntropy keeps vault encryption"
kind: investigation
component: crypto
status: needs-discussion
horizon: someday
origin: discussion
tags: [encryption]
---
# Decide whether ntropy keeps vault encryption

Vault encryption is a large part of ntropy, and its author does not use it.
On 2026-11-17, while deferring asset encryption, the user raised removing
vault encryption again, "as it is a lot of code and complexity, that is not
used at all". This todo holds that question until it is decided.

## Context

Encryption spans the `crypto` component (`src/crypto/`, `src/cipher.rs`,
`src/keys/`), the conversions under `src/migrate/` (`vault encrypt`,
`decrypt`, `rekey`, `passphrase`), the key commands (`unlock`, `lock`), the
`--identity` and `--passphrase-file` flags, and special cases across
reconcile, views, rendering, the language server and the site export. Its
decisions are recorded in the encryption ADRs and
`docs/design/encryption.md`. Several open todos exist only because of it,
asset encryption among them.

## Open questions

- Does anyone else rely on encrypted vaults? Removing it breaks every
  encrypted vault in use; a removal needs a last `vault decrypt` path or a
  release note telling users to decrypt first.
- Removal or a feature flag that keeps the code but builds without it by
  default.
- What the decision means for the encryption todos, which would be dropped
  with it.

## Revisit when

After the first asset release, when the user decides on asset encryption
(todo 01KZ9ZN4GKF9XCT143E1W5YSEJ).
