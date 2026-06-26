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

## Benchmark evidence (2026-06-26)

A reproducible harness now exists (`scripts/benchmark.sh` plus the
`generate_vault` example) that generates a seeded corpus and times every CLI
pattern with `hyperfine`. Measured on an Apple M1 with two views configured
(`by-tag` and `by-status`):

- At 3000 notes, every read/query pattern (list, tag, field, full-text,
  combined, tags, info) lands in 41–49 ms, while `edit`, `reconcile`, and
  `delete` all take 835–865 ms — roughly a 20× gap. Any single mutation costs
  as much as ~20 queries.
- The mutation cost is filesystem-syscall bound, not CPU: the 3000-note rebuild
  spends ~1.2 s of *system* time (unlink + symlink per leaf) against ~0.85 s
  wall. A faster CPU does not move this.
- It scales linearly with note count: the same mutations take ~90 ms at 300
  notes versus ~850 ms at 3000.

The rebuild is O(notes × views): the 20× gap above is with only two views, and
each additional `by-<field>` view adds another full teardown-and-regenerate of
its entire tree per mutation. So the per-write penalty grows with both the note
count and the number of configured views, and field views are exactly the
feature meant to be added freely. This is the concrete pressure that elevates
incremental (or at least per-note partial) link updates from "evaluate" toward
"needed" as soon as a vault runs several views.

## References

- `scripts/benchmark.sh` and `examples/generate_vault.rs` — the harness that
  produced the numbers above; re-run to refresh them or measure a fix.

- ADR 0008 (materialized symlink views) — the literal "incremental" intent.
- ADR 0020 (Unix-only v1 with soft performance target) — why full rebuild is
  acceptable for v1.
