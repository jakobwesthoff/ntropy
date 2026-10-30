# 76. Adjacent terms in a query are an implicit and

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0012](0012-query-dsl-with-hand-rolled-parser.md), whose grammar
joins operands only with `and` and `or`, and
[ADR 0052](0052-client-side-search-as-a-typescript-query-dsl.md), which has
the site's search read two predicates side by side as `and` while the CLI
refuses them.

## Context

`search` joins its trailing arguments with spaces
([ADR 0018](0018-cli-command-surface.md)), so `ntropy search borrow checker`
reaches the parser as `borrow checker`. The parser rejected two operands
without an operator between them with "unexpected trailing input". The
website's query-language page and the agent skill both say that bare words
search the note body, and show `borrow checker` as the example. The site's
search already read adjacent predicates as `and`.

## Decision

Two operands side by side are an `and` that binds like the written one, so
`a b or c` is `(a and b) or c`. This holds after a parenthesised group
(`(a) b`) and before `not`: `not a b` is `(not a) and b`, and `a not b` is
`a and (not b)`.

The CLI parser and the site's CLI-faithful evaluator share the rule, and the
conformance corpus pins it. Every command that takes a selector reads it this
way. `delete` still requires exactly one match.

A phrase is a double-quoted string inside the query. The shell removes plain
double quotes, so a phrase on the command line is written
`ntropy search '"borrow checker"'`; `ntropy search "borrow checker"` passes
the single argument `borrow checker`, which is two words.

## Consequences

- A mistyped operator is searched as a word instead of failing:
  `tag:work nd status:done` searches note bodies for `nd`.
- A keyword where an operand is expected is a search word. `a and and b`
  is `a and text:and and b`, and `and tag:x` is `text:and and tag:x`.
- Two ULIDs side by side are a text query, so `delete <ULID> <ULID>` matches
  nothing and deletes nothing.
- `site --query tag:a tag:b` parses to an `and`, so the sidebar is not
  rooted at a tag; only a single `tag:` predicate roots it
  ([ADR 0056](0056-sidebar-order-labels-landing-notes-and-a-nav-table.md)).
- A trailing operator, an unbalanced parenthesis and a stray `:` remain
  syntax errors.
