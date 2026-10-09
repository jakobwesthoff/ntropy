---
title: "The CLI rejects two-word search that the docs promise works"
kind: bug
component: query
status: needs-discussion
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

## Options

1. Fix the docs to match the CLI. The website example and the skill example
   become `ntropy search '"borrow checker"'` or `ntropy search borrow and
   checker`. No grammar change.
2. Make juxtaposition an implicit `and` in the CLI grammar, matching the site
   search, so the documented example works. This amends ADR 0012, flips the
   parser test, and needs cases in `tests/fixtures/query-corpus.json` so the
   CLI and site evaluators stay in step. The implicit `and` has to bind like
   the explicit one, so `a b or c` parses as `(a and b) or c`.

### Arguments for an implicit `and` (option 2)

- It is what people type. `search` joins its trailing arguments, so
  `ntropy search borrow checker` is the natural spelling without shell quoting.
  Search engines, `fd`, the site search and most issue trackers read a space
  as AND.
- The CLI and the exported site would read the same text the same way, as far
  as the CLI's predicates go, instead of documenting a divergence.
- It fits the grammar. Each bare word is already a `text:` term, so
  `borrow checker` is `text:borrow and text:checker`, and mixed input such as
  `watcher tag:settings` works too. Joining bare words into one phrase instead
  would not compose with predicates. An exact phrase stays available as
  `"borrow checker"`.
- Agents write two-word searches routinely and currently hit the error.
- The most common slip, a forgotten operator in `tag:work status:done`, errors
  today and would do what was meant.

### Arguments for keeping the strict grammar (option 1)

- A mistyped operator fails loudly today. `tag:work nd status:done` errors;
  with an implicit `and` it searches bodies for `nd` and most likely returns
  nothing, with no hint why.
- No grammar change, no ADR amendment, no work in the TypeScript evaluator.
  Only the two docs change.

## Open questions

- Whether the implicit `and` also applies after a parenthesised group (`(a) b`,
  the case in the current parser test). Nothing found so far argues for
  treating it differently.
- `delete` takes the same selector. It still requires exactly one match, so a
  broader reading should not delete more, but that is worth confirming.
- Whether site and CLI should share juxtaposition semantics as a principle,
  recorded in ADR 0052, or only for this case.

## Acceptance

- The website page, the skill reference and the CLI agree on what
  `ntropy search borrow checker` does.
- Whatever the CLI accepts is pinned by a parser test, and for option 2 by
  query-corpus cases.

## Related

The skills repository's `skills/ntropy-todos/SKILL.md` (*Finding work*) tells
agents that two bare words are a syntax error. If option 2 lands, that note
goes.
