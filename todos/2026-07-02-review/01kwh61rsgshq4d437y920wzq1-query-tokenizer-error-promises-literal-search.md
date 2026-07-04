# Query tokenizer error promises "literal" search, but quoting yields regex semantics

Found during the 2026-07-02 codebase review (unit 05, query engine).

## Problem

When the tokenizer hits a character that is not a word character (letters,
digits, `/`, `_`, `-`), it errors with:

> unexpected character `+` (quote it to search literally)

Source: `src/query/token.rs:101-106`.

The advice is wrong. A quoted string is passed to the regex engine verbatim
(`src/query/parser.rs:145` → `Query::Text` → `TextMatcher::new` at
`src/query/text_search.rs:35-41`, which calls `RegexBuilder::new(pattern)`
with no escaping). Quoting does **not** make the search literal:

- `ntropy` query `a + b` → error suggests quoting → `"a + b"` compiles as a
  regex where `+` quantifies the preceding space. It matches `a` followed by
  two-or-more spaces then `b`, and never matches the literal text `a + b`.
- Query `c++` → error suggests quoting → `"c++"` is an *invalid* regex
  (double repetition), so the user following the advice gets a second error:
  `invalid search pattern`.

The design doc (`docs/design/query-and-search.md`) correctly documents `text:`
and quoted phrases as regexes, so the behavior is intended; only the error
message misleads.

## Options

1. Fix the message only, e.g. ``unexpected character `+` (quote the term to
   pass it to the regex engine; escape regex metacharacters with `\`)``.
2. Add a genuinely literal search form (e.g. single quotes, or auto-escaping
   for bare quoted phrases) and keep the message. This is a DSL grammar
   change and needs an ADR amendment (ADR 0012 / 0030).

Option 1 is the minimal correct fix; option 2 is a design decision for the
user.

## Acceptance

- The tokenizer error no longer claims quoting searches literally, or
  quoting actually searches literally.
- A test covering the message/behavior for input like `a + b`.
