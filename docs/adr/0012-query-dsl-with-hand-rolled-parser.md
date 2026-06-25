# 12. Query DSL with hand-rolled parser

Date: 2026-06-24

## Status

Accepted

## Context

Filtering needs boolean expressions (`tag:foo and not tag:bar`) from the start,
not just flags. The DSL should stay small for v1 but be extensible. Parser
options ranged from a parsing crate (winnow, logos+Pratt, chumsky) to a
hand-written one.

## Decision

A query DSL is the single filtering mechanism, unifying `list` and full-text
`search` (full-text is the `text:` predicate).

v1 grammar (precedence `not > and > or`):

- predicates: `tag:value` (segment sub-path match, ADRs 0006/0023),
  `field:value` (equality; membership for list fields), `text:value` (body
  full-text regex)
- any predicate value is a bare word or a double-quoted string, so multi-word
  values are quoted (`status:"in progress"`)
- `text:` and bare-term patterns are regexes evaluated by the embedded grep
  engine with smart-case (ADR 0011); an invalid regex is a query error
- bare-term shorthand: a bare word or quoted string not followed by `:` is a
  `text:` predicate, combinable with operators (`foobar and tag:work`)
- operators: `and`, `or`, `not`, parentheses

Comparison operators (`>`, `<`, `>=`, `<=`) are out of scope for v1.

The parser is hand-written (tokenizer + recursive descent), with no parsing
dependency. It produces positioned errors. Queries are evaluated in the single
scan pass: frontmatter predicates against parsed YAML, `text:` via the embedded
grep engine (ADR 0011).

## Consequences

- One query path for both filtering and search.
- No parser dependency or external API/version churn; the lexer's char handling
  is owned and must be tested.
- Extending to comparisons later adds lexer tokens and one predicate branch.
- v1 has no date-range queries in the DSL.
