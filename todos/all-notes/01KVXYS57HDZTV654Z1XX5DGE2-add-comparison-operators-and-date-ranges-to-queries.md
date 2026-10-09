---
title: "Add comparison operators and date ranges to queries"
kind: feature
component: query
origin: discussion
---
# Add comparison operators and date ranges to queries

Deferred during the v1 design (ADR 0012). The v1 query language has `tag:`,
`field:`, `text:`, bare-term shorthand, and `and`, `or`, `not` with
parentheses, but no comparisons.

## Proposal

Add the `>`, `<`, `>=` and `<=` operators as lexer tokens, with one predicate
branch in the hand-rolled parser. Add date-range queries such as
`created>2026-01-01` and `due<2026-07-01`.

## Open questions

- How are right-hand date literals parsed and compared? This ties into the
  `jiff` choice (ADR 0024).
- Do comparisons apply only to date-typed fields, or also to numeric and
  string fields?
