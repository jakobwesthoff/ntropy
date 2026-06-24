# 9. Generic group-by-field view definitions

Date: 2026-06-24

## Status

Accepted

## Context

Materialized views (ADR 0008) need a definition model: bespoke per-axis code,
one generic mechanism, or scriptable views.

## Decision

A view projects notes into a directory tree keyed by one frontmatter field:

- The grouping key is the field's value.
- A list field (e.g. `tags`) places the note under each value.
- A value containing `/` nests (ADR 0006).
- Each leaf is a relative symlink into `all-notes/` (ADR 0008).

So `by-tag` is the mechanism on `tags`, `by-status` on `status`, etc. Views are
config entries pairing an output directory with a field.

View grouping values are always lowercased and slugified (the same
normalization as tags, ADR 0023), so a field value maps to one canonical
directory regardless of its casing. This is not configurable: case-insensitive
filesystems (default APFS on macOS) cannot hold `Done/` and `done/` as distinct
directories, so a case-preserving mode would behave inconsistently across the
supported platforms.

Leaf links are named `<date>-<slug>.md` (`<date>` = readable creation date from
the ULID). The readable date lives here, in the view, not in the canonical
filename. Same-group `<date>-<slug>` collisions get a short ULID-derived tail.

A note with no value for the field is skipped (found via virtual queries, not a
filesystem bucket).

## Consequences

- A new view axis is a config change naming a field, not new code.
- A note appears in multiple leaves of a list-valued view (intended for tags).
- Views are human-readable and chronologically sortable without ULIDs.
- Per-view filtering depends on the query language, still to be designed.
- Slug/tag normalization and the disambiguator format remain open.
