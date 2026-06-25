# Vault layout and views

This document describes how notes are stored on disk and how materialized
views are derived from them. It consolidates the decisions in
[ADR 0002](../adr/0002-stateless-filesystem-scanning-over-a-derived-index.md),
[ADR 0003](../adr/0003-flat-single-vault-storage-layout.md),
[ADR 0004](../adr/0004-note-identity-and-filename-strategy.md),
[ADR 0006](../adr/0006-hierarchical-tags-by-slash-convention.md),
[ADR 0007](../adr/0007-vault-directory-layout.md),
[ADR 0008](../adr/0008-materialized-symlink-views.md), and
[ADR 0009](../adr/0009-generic-group-by-field-view-definitions.md).

## On-disk structure

A vault is a single directory whose top-level children are each a way to look
at the note set:

    <vault>/
      all-notes/        canonical note files (the source of truth)
      by-tag/           materialized view: symlink tree over the tags field
      by-<field>/       further materialized views, one directory per view
      .ntropy/          configuration / templates (exact use to be decided)

`all-notes/` is the one special directory: it holds the real Markdown files.
Every `by-<field>/` directory holds only symlinks pointing back into
`all-notes/`. Naming the canonical store `all-notes` makes it a sibling of the
views, so the model is uniform: every top-level directory is a projection of
the notes, and `all-notes` is the lossless one.

The filesystem is the only source of truth. ntropy keeps no index or database;
every query walks `all-notes/` and parses frontmatter on demand. There is
nothing to invalidate and no staleness in the data itself; only the derived
views can lag, and only until they are rebuilt.

Only top-level `*.md` files in `all-notes/` are notes. `all-notes/` may also
hold resources (images, attachments) as non-`.md` files or inside
subdirectories; ntropy ignores all of these silently and never traverses
subdirectories for notes. Malformed or badly named top-level `.md` files are
skipped with a stderr warning (`--strict` makes that fatal); see
[ADR 0019](../adr/0019-scan-robustness-and-resource-tolerance.md).

## Canonical note files

Each note in `all-notes/` is named:

    <ulid>-<slug>.md

- `<ulid>` is a 26-character Crockford base32 ULID generated at creation. It is
  fixed-width and is the note's canonical identity. ntropy parses the `id` from
  the filename at read-time and never stores it in frontmatter, so identity has
  exactly one representation and nothing to keep in sync.
- `<slug>` is a normalized form of the note's title. The canonical title lives
  in frontmatter `title` (required); the slug is derived from it and is lossy.

Because the ULID leads the filename and is millisecond-precise, a plain lexical
sort of `all-notes/` is chronological. The readable creation date is not stored
in the canonical filename; it is derived from the ULID and rendered only at
display time.

### Frontmatter

Frontmatter is permissive: any YAML fields the user adds are preserved
untouched and are available for filtering. A set of recognized fields carry
special meaning, currently `title` (required, canonical) and `tags` (a flat
list of strings). Timestamps are derived, never stored: `created` from the
ULID, `modified` from filesystem mtime. `modified` is soft information used for
conveniences like a "recently changed" ordering; nothing important depends on
its accuracy.

### Tags

`tags` is a flat YAML list of strings in which a forward slash denotes
hierarchy by convention, for example:

    tags: [programming/rust, programming/cli, area/work]

ntropy interprets the slash both for prefix filtering and for nesting in
tag-based views.

## Views

A view is the projection of notes into a directory tree keyed by one
frontmatter field. The same mechanism serves every axis:

- The grouping key is the value of the view's field.
- A list-valued field (such as `tags`) places the note under each value, so a
  note appears in several leaves.
- A value containing `/` nests into subdirectories.
- A note with no value for the field is skipped.
- Grouping values are always lowercased and slugified (same normalization as
  tags, ADR 0023), so a value maps to one canonical directory regardless of
  casing. Not configurable: case-insensitive filesystems (default APFS on
  macOS) cannot hold `Done/` and `done/` distinctly.

So `by-tag` is the mechanism applied to `tags`, `by-status` to `status`, and so
on. Views are configuration entries pairing an output directory with a field,
not bespoke code. View definitions are managed through the `view`
CLI commands (`list`/`add`/`edit`/`remove`), not only by hand-editing config.

### Leaf links

Each leaf is a relative symlink named `<date>-<slug>.md`, where `<date>` is the
readable creation date derived from the ULID. The link target is expressed
relative to the vault, for example:

    by-tag/programming/rust/2026-06-24-quarterly-review.md
      -> ../../../all-notes/01JZ4QESXNG8YH6P9V0XYZ-quarterly-review.md

Relative links keep the entire vault relocatable: moving or copying the vault
does not break them. The `../` depth is computed per link from its position in
the view tree. When two notes would collide on `<date>-<slug>` within one
group, a short ULID-derived tail disambiguates them.

Symlinks are used rather than hardlinks: a symlink has an unambiguous canonical
target, can point at the real note anywhere in the tree, and signals that the
entry is derived.

### Freshness

View links are refreshed two ways:

- After any ntropy mutation (create, edit, retitle, delete), so views stay
  current in day-to-day use. In v1 this is implemented as a **full rebuild** of
  the configured view trees rather than true incremental per-link updates: each
  view directory is removed and regenerated from the current note set. This is
  a deliberate, pragmatic deviation from ADR 0008's literal "incremental",
  justified by the soft performance target (ADR 0020); it is always correct and
  prunes stale links for free. Incremental updates are deferred (see
  `todos/`).
- Fully, by `reconcile`, which rebuilds the view trees and catches up after
  edits made outside ntropy (direct `$EDITOR` use, scripts, manual changes).

`reconcile` is also what realigns filenames whose slugs have drifted from their
titles after out-of-band edits. When ntropy itself launches the editor, it
performs that realignment immediately on editor exit.

## Open points

- Per-view filtering (restricting a view to a subset of notes). The query
  language now exists (ADR 0012), but wiring it into view definitions is
  deferred past the v1 view model.

Resolved since first draft: slug/tag normalization and the collision
disambiguator ([ADR 0023](../adr/0023-slug-tag-and-disambiguator-normalization-rules.md));
`.ntropy/` holds config and templates
([ADR 0016](../adr/0016-configuration-format-location-and-vault-resolution.md),
[ADR 0017](../adr/0017-note-templates-with-placeholder-substitution.md));
platform/symlink scope is Unix-only for v1
([ADR 0020](../adr/0020-unix-only-v1-with-soft-performance-target.md)).
