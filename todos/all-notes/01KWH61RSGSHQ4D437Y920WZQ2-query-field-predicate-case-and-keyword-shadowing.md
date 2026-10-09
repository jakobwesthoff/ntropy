# Query field predicates: case-sensitive key lookup contrasts with case-insensitive keywords; `tag`/`text` field names are shadowed

Found during the 2026-07-02 codebase review (unit 05, query engine).

## Problem 1: silent case-sensitivity trap

The predicate *keywords* `tag:` and `text:` are recognized case-insensitively
(`src/query/parser.rs:187-193`, `eq_ignore_ascii_case`), so `Tag:Work` works.
But a generic field predicate looks the field name up in the frontmatter
mapping case-sensitively (`src/query/eval.rs:70`,
`note.frontmatter.get(Value::from(name))`).

Consequence: `Status:done` silently matches nothing against a note whose
frontmatter key is `status`. There is no error and no hint; the user just
gets an empty result set. The inconsistency (keywords forgiving, field names
strict) makes this easy to hit.

YAML keys are case-sensitive, so strict matching is defensible; the issue is
that the two behaviors coexist without documentation and fail silently.

## Problem 2: fields literally named `tag` or `text` cannot be queried

Because any case variant of `tag`/`text` before `:` is captured as the
keyword (`src/query/parser.rs:187-191`), a frontmatter field literally named
`tag` (or `Text`, etc.) is unreachable via a field predicate: `text:foo`
always runs a full-text regex over the body, never a frontmatter comparison.
There is no escape syntax. `docs/design/query-and-search.md` documents the
grammar but does not call out this shadowing.

## Suggested resolution

Decide and document:

- Either make field-name lookup case-insensitive too (consistent forgiveness),
  or keep it strict and state it explicitly in `docs/design/query-and-search.md`
  and the CLI help.
- Document that `tag`/`text` are reserved keys and frontmatter fields with
  those names cannot be targeted by field predicates (or add an escape form,
  which would be a grammar change needing an ADR 0012 amendment).

## Acceptance

- Behavior for mismatched-case field names is a deliberate, documented choice
  (with a test either way).
- The `tag`/`text` reservation is documented where users read about the DSL.
