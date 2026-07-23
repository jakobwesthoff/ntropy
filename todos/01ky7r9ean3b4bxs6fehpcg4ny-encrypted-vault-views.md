# Views for encrypted vaults

Materialized views are disabled in encrypted vaults
([docs/design/encryption.md](../docs/design/encryption.md)): a symlink tree
like `by-tag/` would spell out the tag taxonomy in plaintext names inside the
synced directory, defeating the threat model (the sync/hosting provider must
not read vault structure).

Discussed and parked: relocate view trees for encrypted vaults to a local
cache directory outside the vault (XDG cache), so the feature returns without
anything leaking to the sync provider. Chosen for now instead: disable
entirely, revisit later.

## Open questions

- Cache-directory layout: one tree per vault, keyed how (vault path hash,
  vault id?).
- Lifecycle: when trees are rebuilt and how stale trees for moved or deleted
  vaults are cleaned up.
- Whether `view add` on an encrypted vault errors (current behavior: view
  definitions are inert) or starts targeting the cache location.
- How users discover the relocated trees, since the point of views is
  browsing them in a file manager or shell.
