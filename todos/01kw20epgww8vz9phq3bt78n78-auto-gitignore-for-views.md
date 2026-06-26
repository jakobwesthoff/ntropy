# Auto-manage a .gitignore for materialized views in the vault

Committing a vault to git is a first-class use case (the notes are plain files,
the whole point is filesystem ownership). The materialized view directories
(`by-tag/`, `by-status/`, …) are *derived* symlink trees and should not be
committed: they are regenerated from the frontmatter on demand, so tracking them
is noise and churn, and symlink trees travel badly across machines/platforms.

## What to build

- On `init` (and whenever a view is added), ensure the vault has a `.gitignore`
  that excludes every materialized view's output directory.
- On every `reconcile`, re-check that `.gitignore` for completeness: every
  currently-configured view directory must be present in it. Add any that are
  missing. This keeps the ignore list in sync as views are added/removed.

## Open questions / decisions to make

- One root `.gitignore` listing each view directory, vs. a `.gitignore` dropped
  inside each generated view directory (`by-tag/.gitignore` with `*`). The
  per-directory approach self-cleans when a view is removed and needs no
  knowledge of the other entries; the root approach is a single visible file.
- Respect a user who has intentionally edited the `.gitignore`: only add missing
  view entries, never remove or reorder unrelated lines.
- Should this be opt-out (some users may not use git)? Creating a `.gitignore`
  in a non-git directory is harmless, so likely just always maintain it.
- `.ntropy/` itself: config + templates are worth committing (they travel with
  the vault), so they should NOT be ignored. Only the view output directories.

## Decision context

Surfaced during the 2026-06-26 README restructuring while documenting that a
vault is just files you can commit. The README now states you'll want to ignore
the derived view directories and that ntropy will help with this; this todo is
that help.
