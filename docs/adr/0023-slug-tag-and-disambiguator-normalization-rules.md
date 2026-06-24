# 23. Slug, tag and disambiguator normalization rules

Date: 2026-06-24

## Status

Accepted

## Context

Earlier decisions left slug normalization (ADR 0004), tag normalization (ADR
0006), and the view collision disambiguator (ADR 0009) open.

## Decision

### Slug (title to filename component)

1. German-aware ASCII transliteration: `ä→ae`, `ö→oe`, `ü→ue`, `ß→ss` (and
   uppercase equivalents); other non-ASCII characters best-effort transliterated
   to ASCII, otherwise dropped.
2. Lowercase.
3. Whitespace runs to a single `-`.
4. Remove characters outside `[a-z0-9-]`.
5. Collapse consecutive `-`, trim leading/trailing `-`.
6. Cap at ~72 characters, truncated at a `-` boundary. The full title remains
   in frontmatter.
7. If the result is empty, the slug is `untitled`.

### Tags

Lowercase-normalized. Each `/`-separated segment is normalized with the same
rules as a slug (German-aware ASCII, lowercase, hyphenated); `/` is preserved
as the hierarchy separator. `Rust` and `rust` are the same tag.

### View collision disambiguator

When notes collide on `<date>-<slug>` within a view group, append `-` plus the
trailing N characters of each colliding note's ULID, applied to all colliding
entries. N starts at 3 and increases until every entry is unique, up to the
full 26-character ULID. The trailing (random) portion is used because same-day
collisions share the leading timestamp characters of their ULIDs.

## Consequences

- Filenames and tag directories are ASCII, lowercase, and portable, with
  German titles transliterated readably.
- Tag matching/grouping is case-insensitive by construction.
- Same-day view collisions disambiguate in ~3 characters in practice, without
  exposing full ULIDs unless genuinely required.
