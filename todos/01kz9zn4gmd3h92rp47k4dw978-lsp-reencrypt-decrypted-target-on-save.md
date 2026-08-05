# Write-back for LSP decrypted link targets

In an encrypted vault the language server answers goto-definition, document
links and workspace symbols with a decrypted copy of the target note, written
read-only (`0400`) into the runtime directory, because the real target is
ciphertext and opening it would show binary garbage
([docs/design/encryption.md](../docs/design/encryption.md)).

Following a link therefore lets you read the target but not edit it. Editing
goes back through `ntropy search`. The copies are reused per note identity and
removed when the server shuts down.

Discussed and parked: track the copies the server handed out and re-encrypt
them into the vault on `textDocument/didSave`, so following a link and editing
in place works exactly as in a plaintext vault.

## Open questions

- Two writers. The `ntropy` edit round trip already decrypts, edits and
  re-encrypts; a language server writing the same note concurrently needs the
  same mtime-and-size guard, and the two guards need to agree on what they are
  guarding.
- Lifetime. A decrypted copy handed out for goto-definition currently lives as
  long as the server. A writable copy outlives the jump that created it and has
  no natural close event, since the server never learns the user is finished
  with a buffer it did not open.
- Whether the server should write into a vault at all. It performs no writes
  today, and gaining that capability changes what a compromised or misbehaving
  editor plugin can do.
- What the read-only copies do in the meantime. An editor will let a user type
  into a `0400` buffer and only fail at save time, with no explanation of why.
