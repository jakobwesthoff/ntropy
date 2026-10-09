---
title: "Update only the affected groups after a note mutation"
kind: improvement
component: view
status: needs-discussion
origin: idea
tags: [performance]
---
# Update only the affected groups after a note mutation

Every mutation (`new`, `today`, edit-on-exit, `delete`) calls
`reconcile::refresh_views`. That re-scans the whole vault and diffs every
configured view against its full on-disk tree. Both halves of a mutation's
cost, the note scan (about 45 ms) and the view-tree read walk, are paid in full,
even though one mutation changes exactly one note.

This is "Design B" as built: stateless and always correct, but it does
whole-vault work for a one-note change. "Design A" (targeted updates) was
considered during planning and deferred in favour of Design B's simplicity and
statelessness. Design A is the highest-ceiling option of the view sync
performance set, and also the largest change.

## Proposal

On the mutation path the changed note is known, so sync only the groups that
note participates in, both old and new. This skips the full scan and the full
read walk. Disambiguation is per group (`view::leaf::leaf_names`), so a note's
blast radius stays inside its own groups. That property is what makes this
correct.

`reconcile` stays the full whole-vault pass. Only `refresh_views` becomes
targeted. The approach stays stateless: it derives the old and new groups from
the note itself, not from a cache, and keeps within ADR 0002.

## Open questions

- The note's old group membership is needed. That means snapshotting its
  frontmatter before `$EDITOR` in the editor flow, and parsing the note before
  deletion in `delete`. This brings back the old-state tracking that Design B
  deliberately avoided.
- `refresh_views` (targeted) and `reconcile` (full) would diverge. That leaves
  two code paths to keep correct, instead of today's single one.
- This is the biggest latency win of the set, moving a mutation toward query
  speed or below. It is also the most invasive and the most fragile.
- Is it worth doing? Only if mutation latency becomes a felt problem. At personal
  scale (ADR 0020) the current latency of about 135 ms already feels instant.

## Relations

- Relates to: [Skip link target checks in the post-mutation view sync](01KW4NTNVG8EVGS683HFME4M55-skip-link-target-checks-in-the-post-mutation-view-sync.md),
  same sync path. The trusted mode is the cheaper change on the same path.
- Relates to: [Prune empty view directories during the sync walk](01KW4NTNVG8EVGS683HFME4M56-prune-empty-view-directories-during-the-sync-walk.md),
  same sync path.

## References

- `src/reconcile.rs`: `refresh_views` (mutation path) and `reconcile` (full).
- `src/bin/ntropy/run/mod.rs`: `open_and_refresh` (editor flow, which would need
  the pre-edit snapshot).
- `src/ops/delete.rs`: `delete_note` (would need to parse before removing).
- `src/view/leaf.rs`: per-group disambiguation, which bounds the blast radius.
- ADR 0002 (stateless scan; this stays within it, with no persisted index).
