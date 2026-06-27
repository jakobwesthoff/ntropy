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

Refresh: on ntropy mutations (create/edit/retitle/delete), and on `reconcile`
(catch-up after out-of-band edits).

The refresh is incremental. Each configured view is diffed against its on-disk
tree and only the changed links are touched; a leaf that already points where it
should is left in place. The result is what a from-scratch rebuild would produce
— stale links pruned, always correct — but a mutation pays filesystem writes
proportional to what changed rather than to the whole vault.

## Consequences

- Any tool can browse, open, and search notes by the defined views.
- Views can be stale after out-of-band edits until `reconcile`. Same staleness
  the stateless model already accepts; canonical files are always correct.
- Depends on symlink support, constrained on Windows; couples views to the
  cross-platform decision.
- Mutation commands carry extra logic to update links; relative links require
  computing `../` depth per link.
