# 68. Unique view leaf names per directory, ignoring case

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0023](0023-slug-tag-and-disambiguator-normalization-rules.md),
whose view collision disambiguator separated only notes sharing a base name.

## Context

ADR 0023 appends a ULID tail only to notes whose `<date>-<slug>` names
collide. A tailed name could then equal another note's plain name: `review`
with the all-digit tail `123` against a note whose slug is `review-123`. Or it
could differ from it only in case: slugs are lowercase and tails uppercase, so
`review-FAV` and a slug `review-fav` are two names that a case-insensitive
filesystem, such as default APFS, stores as one entry. With equal names the
view kept one note's leaf and silently dropped the other's; with names
differing only in case, creating the second link failed there and stopped the
view sync.

## Decision

ADR 0023's rule runs first. Then, while two names in one view directory are
equal without regard to case, every base group holding one of them grows its
tail by one character, a plain name gaining a three-character tail, up to the
full ULID. Base names are grouped without regard to case as well. Notes with
distinct ULIDs always end up with distinct names. The site's note pages,
named by the same rule (ADR 0054), get the same guarantee.
