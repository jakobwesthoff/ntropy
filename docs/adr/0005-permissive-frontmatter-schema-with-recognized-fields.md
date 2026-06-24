# 5. Permissive frontmatter schema with recognized fields

Date: 2026-06-24

## Status

Accepted

## Context

Filtering by arbitrary frontmatter fields rules out a fixed closed schema, but
some fields (title, tags, more later) need specific handling. The canonical
title must live somewhere, since the slug derived from it is lossy.

## Decision

Permissive schema. Any user YAML fields are allowed, filterable, and preserved
untouched when ntropy rewrites a note.

On top of that, recognized fields carry special meaning. Established so far:

- `title` (required): canonical title with full case/punctuation/Unicode; the
  slug derives from it. A note without `title` is malformed.
- `tags`: a list of tags (model in ADR 0006).

Timestamps are derived, never stored:

- `created` from the ULID.
- `modified` from filesystem mtime; soft, used only for conveniences like
  "recently changed". Nothing important depends on its accuracy.

## Consequences

- Arbitrary-field filtering works directly: a field is filterable by virtue of
  appearing in frontmatter.
- ntropy can rewrite a note without destroying fields it does not understand.
- Notes missing `title` are malformed and need defined handling, deferred to
  the error-handling decision.
- The recognized-field set grows per feature rather than being replaced.
