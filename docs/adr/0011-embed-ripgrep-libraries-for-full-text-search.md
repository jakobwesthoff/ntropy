# 11. Embed ripgrep libraries for full-text search

Date: 2026-06-24

## Status

Superseded by [ADR 0030](0030-replace-ripgrep-stack-with-regex-crate-for-full-text-search.md).

## Context

Full-text search over note bodies should be ripgrep-fast. Options: embed
ripgrep's own crates, shell out to the `rg` binary, or use the `regex` crate
directly.

## Decision

Embed ripgrep's libraries (`grep-searcher`, `grep-regex`; `ignore` for a
parallel walk if useful) in the binary. Text predicates are evaluated against
each note's in-memory body during the single scan pass.

## Consequences

- Real ripgrep performance with no runtime dependency; the tool stays a single
  self-contained binary.
- Search composes with frontmatter filtering in one pass (no separate grep
  invocation).
- More crates to track than a plain `regex` dependency.
