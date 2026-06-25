# 6. Hierarchical tags by slash convention

Date: 2026-06-24

## Status

Accepted

## Context

Tags are the primary axis for filtering and for tag-based views, which should
nest, without forcing complex YAML on a hand-author.

## Decision

`tags` is a flat YAML list of strings. A forward slash denotes hierarchy by
convention:

    tags: [programming/rust, programming/cli, area/work]

ntropy interprets the slash for filtering and for nested view directories
(`programming/rust` → `programming/rust/`). The slash maps onto directories and
matches the Obsidian convention. A nested YAML structure was rejected as harder
to author and parse with no advantage.

Filter matching is a segment sub-path match (refined in
[ADR 0023](0023-slug-tag-and-disambiguator-normalization-rules.md) and the
query design): a query's `/`-separated segments match a tag when they occur as a
contiguous run of full segments anywhere within that tag, so `programming`
matches `programming`, `programming/rust` and `area/programming`. This is
broader than an ancestor-only prefix match.

## Consequences

- Nested views fall directly out of the tag string; no separate hierarchy
  mechanism.
- Frontmatter stays a simple list of strings.
- A tag cannot contain a literal slash that is not a hierarchy level. Tag
  normalization rules remain open.
