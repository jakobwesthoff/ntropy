# 32. Auto-managed .gitignore for view directories

Date: 2026-06-27

## Status

Accepted

Extends the reserved-name set of
[ADR 0007](0007-vault-directory-layout.md) and builds on the derived view
directories of [ADR 0008](0008-materialized-symlink-views.md).

## Context

A vault is plain files (ADR 0003), so committing it to git is a first-class
use case. The materialized view directories are derived symlink trees (ADR
0008), regenerated from frontmatter on demand: tracking them is churn, and
symlink trees travel badly across machines. Keeping them out of git was left
to the user, who had to know which directories were derived and maintain the
ignore list by hand as views came and went.

## Decision

ntropy maintains a single root `<vault>/.gitignore` whose managed entries
mirror exactly the configured views.

- Each managed entry is the view directory anchored to the vault root,
  `/<name>/`, derived from the view's layout path so this stays in step with
  ADR 0007 rather than re-encoding it. A generic comment,
  `# ntropy: derived view directory, safe to ignore`, sits directly above each
  entry and doubles as an ownership marker.
- Every view-affecting operation syncs the file: `init`, `reconcile` (and the
  lighter view refresh after `new`/`today`/edit), and `view add`/`view
  remove`. A sync adds entries for configured views not yet present and prunes
  managed entries whose view is no longer configured.
- Ownership is strict for pruning, tolerant for adding. An entry is pruned
  only when it carries the marker above it; a line the user wrote is never
  removed or reordered, even when it names the same directory. Conversely, a
  configured view is considered already ignored when any line names its
  directory in any slash form, so a user's hand-added ignore is not
  duplicated.
- ntropy never deletes a directory. Removing a view prunes its ignore entry
  but leaves the now-stale directory on disk, and the command reports it so
  the user can delete it.
- `.gitignore` joins `all-notes` and `.ntropy` as a reserved name, so a view
  cannot be named after the file ntropy manages.

## Consequences

- Committing a vault keeps the derived view trees out of git without manual
  upkeep; the ignore list tracks views as they are added and removed.
- A removed view's directory remains and, with its ignore entry pruned,
  becomes visible to git; this is surfaced in the command output rather than
  silently cleaned up.
- Pruning the last view can leave an empty or comment-only `.gitignore`; it is
  left in place, never deleted.
- A user who forges the exact marker comment above a non-view entry would have
  that entry treated as ntropy-owned and pruned on the next sync; the marker
  is documentation, not a security boundary.
