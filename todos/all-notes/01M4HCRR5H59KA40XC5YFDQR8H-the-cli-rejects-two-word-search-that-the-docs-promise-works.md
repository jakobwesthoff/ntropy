---
title: "The CLI rejects two-word search that the docs promise works"
kind: bug
component: query
origin: implementation
---
# The CLI rejects two-word search that the docs promise works

Two places tell users that bare words search note bodies directly, and the CLI
rejects that query. The mismatch was found while writing the ntropy-todos skill
in the skills repository, where a two-word duplicate search failed.

## Problem

Two places tell users that bare words search note bodies directly:

- the website's query-language page
  (`docs/website/all-notes/01M28Q32FC8RW01C5DSEEV77DW-query-language.md`,
  the example block and the *Terms* section): `ntropy search borrow checker`
  "looks for that text directly"
- the agent skill (`skills/ntropy/references/querying.md`, the query-language
  examples): `ntropy search -n borrow checker  # bare words = full-text over the body`

The CLI rejects that query (ntropy 2.1.1, checked 2026-10-10):

    $ ntropy search -n borrow checker
    error: while resolving the selector: query syntax error at position 7: unexpected trailing input

`search` joins its trailing arguments into one string (ADR 0018), and the
parser does not accept two terms side by side. That is deliberate. ADR 0012
defines the bare-term shorthand as a `text:` predicate "combinable with
operators", and `juxtaposed_terms_without_operator_is_error` in
`src/query/parser.rs` asserts that juxtaposition is not an implicit `and`. A
single bare word, a quoted phrase (`'"borrow checker"'`) and `borrow and checker`
all work.

The website's client-side search reads the same input differently.
`docs/design/site-frontend.md` says two predicates side by side are joined by
`and` (ADR 0052, amended). The CLI and the site therefore disagree on
`entrance anim` as well.

## Proposal

Make juxtaposition an implicit `and` in the CLI grammar, matching the site
search, so the documented example works. This amends ADR 0012, flips the
parser test, and needs cases in `tests/fixtures/query-corpus.json` so the CLI
and site evaluators stay in step.

- The implicit `and` binds like the explicit one, so `a b or c` parses as
  `(a and b) or c`.
- It also applies after a parenthesised group (`(a) b`, the case in the
  current parser test).
- `delete` takes the same selector and keeps requiring exactly one match, so
  the broader reading deletes nothing more.
- Juxtaposition semantics are a shared principle of CLI and site. ADR 0052
  gains the amendment link when the work lands.

Each bare word is already a `text:` term, so `borrow checker` is
`text:borrow and text:checker`, and mixed input such as `watcher tag:settings`
works too. An exact phrase stays available as `"borrow checker"`. The cost is
that a mistyped operator, as in `tag:work nd status:done`, no longer errors:
it searches bodies for `nd`.

## Decisions

- 2026-10-30, user: option 2, two terms side by side are an implicit `and` that binds like the explicit one, matching the site search. Option 1 (fix the docs to match the strict CLI) is dropped.
- 2026-10-30, lead: the implicit `and` also applies after a parenthesised group (`(a) b`); `delete` keeps requiring exactly one match; juxtaposition semantics are recorded as a shared principle of CLI and site, and ADR 0052 gains the amendment link when the work lands.

## Acceptance

- The website page, the skill reference and the CLI agree on what
  `ntropy search borrow checker` does.
- The parser test and query-corpus cases pin the implicit `and`.

## Related

The skills repository's `skills/ntropy-todos/SKILL.md` (*Finding work*) tells
agents that two bare words are a syntax error. When this lands, that note
goes.
