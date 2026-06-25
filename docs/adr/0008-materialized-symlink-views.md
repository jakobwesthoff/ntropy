# 8. Materialized symlink views

Date: 2026-06-24

## Status

Accepted

## Context

`list`/`search` are virtual views. Open question: also project views onto the
filesystem as browsable trees so other tools (neovim, file managers, `grep`)
can navigate notes by tag/date/field. The author works primarily in neovim,
where external-tool navigation is the payoff.

## Decision

ntropy materializes views as symlink trees. Each defined view is a top-level
vault directory (ADR 0007) whose structure mirrors the view's organization and
whose leaves are symlinks into `all-notes/`.

Symlinks are relative to the vault (e.g. `../../all-notes/<ulid>-<slug>.md`),
so the vault can be moved or copied without breaking links. Hardlinks were
rejected: they cannot span filesystems, cannot point at directories, and make
"which is the real note" ambiguous.

Refresh: on ntropy mutations (create/edit/retitle/delete), and fully on
`reconcile` (catch-up after out-of-band edits).

v1 note: the post-mutation refresh is implemented as a full rebuild of the
configured view trees (remove + regenerate), not true incremental per-link
updates. This is a deliberate simplification under the soft performance target
(ADR 0020); it is always correct and prunes stale links. Incremental updates
are deferred and tracked in `todos/`.

## Consequences

- Any tool can browse, open, and search notes by the defined views.
- Views can be stale after out-of-band edits until `reconcile`. Same staleness
  the stateless model already accepts; canonical files are always correct.
- Depends on symlink support, constrained on Windows; couples views to the
  cross-platform decision.
- Mutation commands carry extra logic to update links; relative links require
  computing `../` depth per link.
