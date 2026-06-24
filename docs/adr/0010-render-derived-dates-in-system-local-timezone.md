# 10. Render derived dates in system local timezone

Date: 2026-06-24

## Status

Accepted

## Context

`created` (from the ULID) is a UTC instant; `modified` is filesystem mtime.
Rendering them to `YYYY-MM-DD` (list output, view leaf names) needs a
timezone, but only near-midnight notes are sensitive. Sorting/filtering use the
UTC instant directly and need no timezone.

## Decision

Derived dates render in the machine's system local timezone at render time, for
both display and the `<date>` in view leaf names. User-authored frontmatter
date fields (e.g. `due`) are literal strings filtered as written and are
outside this decision.

## Consequences

- Displayed dates match the author's lived sense of when a note was written.
- Running `reconcile` in a different timezone can shift a near-midnight note's
  leaf date by a day. Self-healing, affects only the display name, accepted;
  rendering is not deterministic across timezones.
- Depends on a correct system timezone.
