---
title: "Skip link target checks in the post-mutation view sync"
kind: improvement
component: view
origin: idea
tags: [performance]
---
# Skip link target checks in the post-mutation view sync

A noop view sync spends most of its time in the view-tree read walk in
`collect_state` (`src/view/materialize.rs`). The walk issues one `readlink` per
leaf to compare each leaf's stored target with the desired one. With a few
thousand notes and a few tags each, that is about 10k `readlink` syscalls per
sync. That is an order of magnitude more than the per-directory `readdir`
calls, and the largest sync-specific cost. The note scan (about 45 ms) is the
other half.

After ntropy's own mutation, the tree was last written by ntropy, so a leaf that
exists under the correct name almost always has the correct target. The
exception is out-of-band tampering. That is exactly what `reconcile` repairs,
and ADR 0008 already accepts that views can be stale after out-of-band edits
until `reconcile`.

## Proposal

Give `sync_view` two comparison modes:

- Presence-only (trusted): compare the desired leaf names with the actual names,
  create missing leaves and remove extra ones. This mode skips `readlink`
  entirely. `refresh_views`, the post-mutation path, uses it.
- Verify (thorough): also read and compare each leaf's target, correcting drift.
  `reconcile` uses it.

This removes about 10k `readlink` syscalls from the hot path, and `reconcile`
stays the full repair.

## Open questions

- A leaf with the correct name and a wrong target, from out-of-band tampering, is
  then fixed only by `reconcile`, not by the next mutation. This matches the
  existing staleness contract. Is that acceptable?
- What is the API shape: a mode enum or a bool on `sync_view` and `sync_all`,
  threaded from `refresh_views` and `reconcile`?

## Done when

Most of the existing `sync_view` test matrix carries over. Add cases asserting
that the trusted mode does not correct a drifted target, while verify mode does.

## References

- `src/view/materialize.rs`: `sync_view`, `collect_state` (the `read_link` per
  leaf).
- `src/reconcile.rs`: `refresh_views` (trusted) and `reconcile` (verify), and
  `sync_views_and_gitignore`.
- ADR 0008 (views may be stale after out-of-band edits until `reconcile`).
