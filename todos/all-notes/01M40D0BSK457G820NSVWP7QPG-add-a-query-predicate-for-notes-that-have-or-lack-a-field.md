---
title: "Add a query predicate for notes that have or lack a field"
kind: feature
component: query
---
# Add a query predicate for notes that have or lack a field

Finding notes that still need classifying is a common clean-up step: notes
without `status`, without `lang`, or without any tags at all. The query
language cannot ask for that directly.

## Goal

The predicates today are `tag:`, `text:` and `field:value` (`src/query/ast.rs`).
`field:value` compares a concrete value and returns `false` when the field is
absent (`src/query/eval.rs`, `field_matches`). The closest workaround is
`not field:value` for some value the field might hold. That matches notes
lacking the field and notes holding any other value alike. No form selects
exactly the notes without `status`, or exactly the notes with an empty or
missing `tags` list.

## Proposal

A predicate that tests for the presence of a frontmatter field, usable with
`not` for absence. Candidate spellings:

- `has:lang`, with `not has:lang` for the missing case
- `missing:lang` as the direct form
- a dedicated `untagged` for notes with no tags, or `not has:tags`

Presence should probably mean "the key exists with a non-empty value", so
that `tags: []` and `status: ""` count as missing. That needs deciding.

## Open questions

- Keyword collision. The parser treats `tag` and `text` as reserved keys, and
  every other `word:` as a frontmatter field (`src/query/parser.rs`, the
  `tag`/`text` branch in predicate parsing). `has:lang` parses today as the
  field `has` with value `lang`. Reserving `has` or `missing` changes the
  meaning of queries against a field of that name, and leaves such a field
  queryable by no syntax at all. Alternatives that avoid the collision: a sigil
  (`lang:*` for presence), or a form the grammar cannot produce today.
- Site search. ADR 0052 has the TypeScript evaluator in `site/src/search/`
  mirror the CLI's predicates, with the shared corpus
  `tests/fixtures/query-corpus.json` keeping them in step. The new predicate
  has to land in both, with corpus cases.
- Interaction with field defaults. If per-vault defaults land, this predicate
  should test the note's own frontmatter. Otherwise a defaulted field would
  always count as present, and the notes that rely on the default could not
  be found.
- ADR. The grammar is decided in ADR 0012. A new predicate kind is a grammar
  change and wants an ADR amendment or a new ADR.

## Relations

- Relates to: [Let vaults set default values for fields that notes lack](01M40D0BSK457G820NSVWP7QPF-let-vaults-set-default-values-for-fields-that-notes-lack.md), because the predicate must see the raw frontmatter, not the defaulted values
