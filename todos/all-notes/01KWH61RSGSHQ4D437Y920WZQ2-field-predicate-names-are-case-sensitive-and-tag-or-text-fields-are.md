---
title: "Field predicate names are case-sensitive and tag or text fields are unreachable"
kind: bug
component: query
origin: review
tags: [ux]
---
# Field predicate names are case-sensitive and tag or text fields are unreachable

Found in the 2026-07-02 codebase review of the query engine. The keywords `tag:` and `text:` match in any letter case, but generic field predicates look up frontmatter keys case-sensitively, so `Status:done` silently matches nothing. A frontmatter field named `tag` or `text` cannot be targeted by a field predicate at all. Both behaviours coexist without documentation.

## Problem

### Case-sensitive field lookup

The parser recognizes `tag:` and `text:` in any letter case (`src/query/parser.rs`, around lines 187 to 193, using `eq_ignore_ascii_case`), so `Tag:Work` works. A generic field predicate looks its name up in the frontmatter mapping case-sensitively (`src/query/eval.rs`, around line 70). So `Status:done` matches nothing against a note with a `status` key, and the query returns an empty result with no error and no hint.

YAML keys are case-sensitive, so strict matching would be defensible, but the keywords already ignore case and the two behaviours sit side by side without documentation. Field lookup becomes case-insensitive to match the keywords.

### Shadowed field names

Any case variant of `tag` or `text` before the colon is captured as the keyword (`src/query/parser.rs`, around lines 187 to 191). A frontmatter field literally named `tag` is therefore unreachable: `text:foo` always runs a full-text regex over the body and never compares frontmatter. The grammar has no escape syntax. `docs/design/query-and-search.md` documents the grammar but does not mention this shadowing.

## Impact

A query whose field name differs in case from the frontmatter key returns nothing and says nothing. A frontmatter field named `tag` or `text` cannot be queried.

## Open questions

- Should `tag` and `text` frontmatter fields get an escape form, or be documented as reserved keys? An escape form is a grammar change that needs an amendment to ADR 0012.

## Decisions

- 2026-10-30, user: field lookup ignores case, like the keywords.

## Done when

- Field-name lookup ignores letter case, pinned by a test, and the behaviour is documented in `docs/design/query-and-search.md` and the CLI help.
- The `tag` and `text` reservation is documented where users read about the query language, or an escape form exists.
