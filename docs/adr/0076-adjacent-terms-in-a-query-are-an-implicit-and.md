# 76. Adjacent terms in a query are an implicit and

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0012](0012-query-dsl-with-hand-rolled-parser.md), whose grammar
joins operands only with `and` and `or`;
[ADR 0018](0018-cli-command-surface.md), which joins the trailing arguments of
`search` into one string; and
[ADR 0052](0052-client-side-search-as-a-typescript-query-dsl.md), which has the
site's search read two predicates side by side as `and` while the CLI refuses
them.

## Context

`ntropy search borrow checker` reached the parser as `borrow checker`, and the
parser rejected two operands without an operator between them with
"unexpected trailing input". The website's query-language page and the agent
skill both say that bare words search the note body. The site's search already
read adjacent predicates as `and`.

A shell removes plain double quotes, so `ntropy search "borrow checker"` also
reaches ntropy as the arguments `borrow checker`, indistinguishable from two
words typed separately.

## Decision

Two operands side by side are an `and` that binds like the written one, so
`a b or c` is `(a and b) or c`. This holds after a parenthesised group
(`(a) b`) and before `not`: `not a b` is `(not a) and b`, and `a not b` is
`a and (not b)`. The CLI parser and the site's CLI-faithful evaluator share
the rule, and the conformance corpus pins it.

Every command that takes a query from trailing arguments (`search`, with its
aliases `list` and `edit`, `delete`, `render` and `site`) reads each argument
on its own and joins them:

- An argument without whitespace is used as typed: `tag:work`, `or`, a word.
- An argument with whitespace and no query syntax is a phrase. Query syntax
  is a `:`, a parenthesis, a double quote, or one of the words `and`, `or`,
  `not` in any letter case. A phrase matches its words in order with any
  whitespace between them, a line break included, and every other character
  literally, so `"c++ tips"` and `"node.js tips"` are not patterns. It
  follows the case rule of every text term: lowercase matches regardless of
  case, an uppercase letter matches exactly.
- An argument with whitespace and query syntax is a query. Beside other
  arguments it is wrapped in parentheses when it is complete on its own, so
  `search 'a or b' c` is `(a or b) and c`. A piece of a query split at a
  space (`'and not status:done'`, `'tag:a or'`, `'(tag:a or'`) joins as it
  is.
- Blank arguments are dropped and the others trimmed. `search ""` lists
  every note.

So `ntropy search "borrow checker"` finds the phrase, `ntropy search borrow
checker` finds notes holding both words in any order, and a whole query in
one quoted argument, `ntropy search 'tag:work and not status:done'`, works as
before. `delete` still requires exactly one match.

A phrase that contains query syntax is written as a quoted string inside the
query, `ntropy search '"page not found"'`. That quoted form is a plain
regular expression, so it does not match across a line break.

## Consequences

- A mistyped operator is searched as a word instead of failing:
  `tag:work nd status:done` searches note bodies for `nd`.
- A keyword where an operand is expected is a search word. `a and and b`
  is `a and text:and and b`, and `and tag:x` is `text:and and tag:x`.
- `search tag:work and 'status:open or status:new'` means
  `tag:work and (status:open or status:new)`, where the previous plain join
  made it `(tag:work and status:open) or status:new`.
- A syntax error in a call with several arguments reports its position in
  the query ntropy built from them, the string that "no note matches" and
  "is ambiguous" messages show as well.
- Two ULIDs side by side are a text query, so `delete <ULID> <ULID>` matches
  nothing and deletes nothing.
- `site --query tag:a tag:b` parses to an `and`, so the sidebar is not
  rooted at a tag; only a single `tag:` predicate roots it
  ([ADR 0056](0056-sidebar-order-labels-landing-notes-and-a-nav-table.md)).
- A trailing operator, an unbalanced parenthesis and a stray `:` remain
  syntax errors.
