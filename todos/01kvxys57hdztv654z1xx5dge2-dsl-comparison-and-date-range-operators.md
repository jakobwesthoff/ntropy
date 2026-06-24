# Query DSL comparison & date-range operators (post-v1)

Deferred from v1 per ADR 0012. The v1 DSL has `tag:`, `field:`, `text:`,
bare-term shorthand, and `and`/`or`/`not`/parens, but no comparisons.

## To decide later

- Add `>`, `<`, `>=`, `<=` operators (lexer tokens + one predicate branch in
  the hand-rolled parser).
- Date-range queries (`created>2026-01-01`, `due<2026-07-01`), including how RHS
  date literals are parsed and compared (ties into the `jiff` choice, ADR 0024).
- Whether comparisons apply only to date-typed fields or also numeric/string.
