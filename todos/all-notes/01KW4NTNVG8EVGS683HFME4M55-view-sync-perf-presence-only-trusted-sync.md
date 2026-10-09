# view-sync-perf: presence-only comparison on the trusted post-mutation sync

## Status

Open (idea, not yet designed).

## Category

`view-sync-perf` — optimizations to the incremental view sync
(`view::sync_view`, ADR 0008). See sibling `view-sync-perf-*` todos.

## Context

A noop sync's cost is dominated by the view-tree read-walk in
`collect_state` (`src/view/materialize.rs`), which issues **one `readlink` per
leaf** to compare each leaf's stored target against the desired one. With a
few thousand notes and a few tags each, that is ~10k `readlink` syscalls per
sync — an order of magnitude more than the per-directory `readdir` calls, and
the single largest sync-specific cost (the note scan, ~45 ms, is the other
half).

After ntropy's *own* mutation, the tree was last written by ntropy, so a leaf
that exists under the correct name almost always has the correct target. The
only exception is out-of-band tampering — which is exactly what `reconcile`
exists to repair, and ADR 0008 already accepts that views can be stale after
out-of-band edits until `reconcile`.

## Idea

Give `sync_view` two comparison modes:

- **Presence-only (trusted):** compare desired leaf *names* against actual
  names; create missing, remove extra. Skip `readlink` entirely. Used by
  `refresh_views` (the post-mutation path).
- **Verify (thorough):** also read and compare each leaf's target, correcting
  drift. Used by `reconcile`.

This removes ~10k `readlink` syscalls from the hot path while keeping
`reconcile` as the full repair.

## Trade-offs / open questions

- A same-name-but-wrong-target leaf (out-of-band tamper) is then only fixed by
  `reconcile`, not by the next mutation. Consistent with the existing staleness
  contract, but confirm that's acceptable.
- API shape: a mode enum/bool on `sync_view`/`sync_all`, threaded from
  `refresh_views` vs `reconcile`.
- Most of the existing `sync_view` test matrix carries over; add cases asserting
  the trusted mode does NOT correct a drifted target while verify mode does.

## References

- `src/view/materialize.rs` — `sync_view`, `collect_state` (the `read_link`
  per leaf).
- `src/reconcile.rs` — `refresh_views` (trusted) vs `reconcile` (verify),
  `sync_views_and_gitignore`.
- ADR 0008 (views may be stale after out-of-band edits until `reconcile`).
