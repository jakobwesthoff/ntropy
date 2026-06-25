# View freshness: evaluate incremental link updates

## Status

Deferred (post-v1 evaluation).

## Context

ADR 0008 specifies that materialized symlink views are refreshed two ways:
incrementally during normal ntropy mutations (create/edit/retitle) and fully by
`reconcile`. The v1 implementation deliberately deviates: after *any* mutating
operation (new, edit, delete) ntropy re-runs the **full view rebuild**
(`reconcile`'s remove+regenerate of every configured view directory) rather than
performing true incremental per-link updates.

This is a pragmatic simplification justified by the soft performance target in
ADR 0020 (personal-scale vaults, low thousands of notes, no hard latency SLA).
A full rebuild is simpler, always correct, and prunes stale/orphaned links for
free, at the cost of doing O(all notes × all views) work on every mutation.

## Task

Evaluate whether full-rebuild-after-mutation remains adequate for real vault
sizes, or whether true incremental link updates (touch only the links the
mutated note participates in) are warranted. Decide and, if pursued, design the
incremental update path.

## References

- ADR 0008 (materialized symlink views) — the literal "incremental" intent.
- ADR 0020 (Unix-only v1 with soft performance target) — why full rebuild is
  acceptable for v1.
