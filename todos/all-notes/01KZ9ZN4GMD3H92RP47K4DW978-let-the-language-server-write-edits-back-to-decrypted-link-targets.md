---
title: "Let the language server write edits back to decrypted link targets"
kind: feature
component: lsp
horizon: someday
origin: discussion
tags: [encryption]
---
# Let the language server write edits back to decrypted link targets

In an encrypted vault the language server answers goto-definition, document
links and workspace symbols with a decrypted copy of the target note. The copy
is written read-only (`0400`) into the runtime directory, because the real
target is ciphertext and opening it would show binary garbage
(`docs/design/encryption.md`).

Following a link therefore lets you read the target but not edit it. Editing
goes back through `ntropy search`. The copies are reused per note identity and
removed when the server shuts down.

## Proposal

This was discussed and parked. The server would track the copies it handed
out and re-encrypt them into the vault on `textDocument/didSave`. Following a
link and editing in place would then work as in a plaintext vault.

## Open questions

- Two writers. The `ntropy` edit round trip already decrypts, edits and
  re-encrypts. A language server writing the same note concurrently needs the
  same mtime-and-size guard, and the two guards need to agree on what they
  guard.
- Lifetime. A decrypted copy handed out for goto-definition currently lives as
  long as the server. A writable copy outlives the jump that created it and has
  no natural close event, since the server never learns that the user is done
  with a buffer it did not open.
- Whether the server should write into a vault at all. It performs no writes
  today, and gaining that capability changes what a compromised or misbehaving
  editor plugin can do.
- What the read-only copies do in the meantime. An editor lets a user type
  into a `0400` buffer and only fails at save time, with no explanation of why.

## Revisit when

No trigger known; reconsider at the next sweep.
