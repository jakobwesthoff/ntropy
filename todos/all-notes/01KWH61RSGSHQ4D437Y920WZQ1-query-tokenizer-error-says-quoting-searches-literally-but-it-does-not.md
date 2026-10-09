---
title: "Query tokenizer error says quoting searches literally, but it does not"
kind: bug
component: query
origin: review
tags: [error-handling]
---
# Query tokenizer error says quoting searches literally, but it does not

Found in the 2026-07-02 codebase review of the query engine. When the tokenizer meets a character outside letters, digits, `/`, `_` and `-`, it fails with "unexpected character `+` (quote it to search literally)". The advice is wrong: a quoted string goes to the regex engine unescaped, so quoting never makes a search literal. The error is raised in `src/query/token.rs` (around line 101).

## Problem

Established by reading the code during the review. A quoted string becomes `Query::Text` (`src/query/parser.rs`, around line 145), and `TextMatcher::new` in `src/query/text_search.rs` passes it to `RegexBuilder::new` without escaping. The design doc `docs/design/query-and-search.md` documents `text:` and quoted phrases as regexes, so the behaviour is intended. Only the error message is wrong. Two cases:

- `a + b`: the error suggests quoting. `"a + b"` compiles as a regex in which `+` repeats the preceding space. It matches `a`, then two or more spaces, then `b`, and never matches the literal text `a + b`.
- `c++`: the error suggests quoting. `"c++"` is an invalid regex (a double repetition), so the user gets a second error, `invalid search pattern`.

## Impact

A user who follows the advice gets either no matches or an `invalid search pattern` error.

## Suggested fix

Fix the message. For example: ``unexpected character `+` (quote the term to pass it to the regex engine; escape regex metacharacters with `\`)``.

## Open questions

A genuinely literal search form, such as single quotes or automatic escaping of quoted phrases, would change the query grammar and need an amendment to ADR 0012 and ADR 0030. That is a design decision for the user and is not needed for this fix.

## Done when

- The tokenizer error no longer claims that quoting searches literally.
- A test covers the message for input such as `a + b`.
