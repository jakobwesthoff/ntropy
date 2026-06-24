# 2. Stateless filesystem scanning over a derived index

Date: 2026-06-24

## Status

Accepted

## Context

Notes are authored in an arbitrary `$EDITOR`, outside ntropy's control, so any
state ntropy holds about note contents can drift from disk between
invocations. Choice: a stateless scanner versus a derived index (e.g. SQLite
with full-text search).

## Decision

Stateless. The filesystem is the only source of truth. Every `list`/`search`/
`filter` query walks the note directory and parses frontmatter on demand. No
persistent index, database, or daemon.

## Consequences

- No staleness, cache-coherency, or index-corruption failure modes, and no
  invalidation machinery to build.
- Query cost scales with vault size on every invocation. Acceptable at
  personal scale (hundreds to low thousands of notes).
- Not aimed at 100k+ vaults or ranked relevance. An optional cache can be
  added later, since the filesystem stays canonical regardless.
