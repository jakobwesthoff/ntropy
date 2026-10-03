# Query predicate for notes that have, or lack, a field

## Why

Finding notes that still need classifying is a common clean-up step: notes
without `status`, without `lang`, or without any tags at all. The query
language cannot ask that directly. Its predicates are `tag:`, `text:` and
`field:value` (`src/query/ast.rs`), and `field:value` compares a concrete
value, returning `false` when the field is absent (`src/query/eval.rs`,
`field_matches`).

The closest workaround is `not field:value` for some value the field might
hold, which matches notes lacking the field and notes holding any other
value alike. There is no form that selects exactly the notes without
`status`, or exactly the notes with an empty or missing `tags` list.

## Scope

A predicate that tests for the presence of a frontmatter field, usable with
`not` for absence. Candidate spellings:

- `has:lang`, with `not has:lang` for the missing case
- `missing:lang` as the direct form
- a dedicated `untagged` for notes with no tags, or `not has:tags`

Presence should probably mean "the key exists with a non-empty value", so
that `tags: []` and `status: ""` count as missing. That needs deciding.

## Open questions

- **Keyword collision.** The parser treats `tag` and `text` as reserved keys
  and every other `word:` as a frontmatter field
  (`src/query/parser.rs`, the `tag`/`text` branch in predicate parsing).
  `has:lang` parses today as the field `has` with value `lang`. Reserving
  `has` or `missing` changes the meaning of queries against a field of that
  name, and leaves such a field queryable by no syntax at all. Alternatives
  that avoid the collision: a sigil (`lang:*` for presence), or a form the
  grammar cannot produce today.
- **Site search.** ADR 0052 has the TypeScript evaluator in
  `site/src/search/` mirror the CLI's predicates, with the shared corpus
  `tests/fixtures/query-corpus.json` keeping them in step. The new predicate
  needs to land in both, with corpus cases.
- **Interaction with field defaults.** If per-vault defaults land (todo
  `01m40d0bsk457g820nsvwp7qpf`), this predicate should test the note's own
  frontmatter. Otherwise a defaulted field would always count as present,
  and the notes that rely on the default could not be found.
- **ADR.** The grammar is decided in ADR 0012. A new predicate kind is a
  grammar change and wants an ADR amendment or a new ADR.
