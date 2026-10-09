---
title: "Show views in encrypted vaults"
kind: feature
component: view
horizon: someday
origin: discussion
tags: [encryption]
---
# Show views in encrypted vaults

Materialized views are disabled in encrypted vaults (`docs/design/encryption.md`).
A symlink tree like `by-tag/` would spell out the tag taxonomy in plaintext
names inside the synced directory, which defeats the threat model: the
sync or hosting provider must not read the vault structure.

## Proposal

Relocate the view trees of an encrypted vault to a local cache directory
outside the vault (XDG cache). The feature then returns without anything
leaking to the sync provider. This was discussed and parked. For now the
views are disabled entirely.

## Open questions

- Cache-directory layout: one tree per vault, keyed how (vault path hash,
  vault id?).
- Lifecycle: when trees are rebuilt, and how stale trees for moved or
  deleted vaults are cleaned up.
- Whether `view add` on an encrypted vault errors (current behavior: view
  definitions are inert) or starts targeting the cache location.
- How users find the relocated trees, since the point of views is browsing
  them in a file manager or shell.

## Revisit when

No trigger known; reconsider at the next sweep.
