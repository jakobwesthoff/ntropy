# Query and search

How ntropy filters and searches notes. Consolidates
[ADR 0002](../adr/0002-stateless-filesystem-scanning-over-a-derived-index.md),
[ADR 0005](../adr/0005-permissive-frontmatter-schema-with-recognized-fields.md),
[ADR 0006](../adr/0006-hierarchical-tags-by-slash-convention.md),
[ADR 0011](../adr/0011-embed-ripgrep-libraries-for-full-text-search.md), and
[ADR 0012](../adr/0012-query-dsl-with-hand-rolled-parser.md).

## One mechanism

Filtering and full-text search are the same thing: a query expression. There
is no separate "filter" versus "search" path. Full-text is just the `text:`
predicate. A `search` command, if provided, is sugar for a bare `text:` query.

## v1 grammar

Precedence: `not` > `and` > `or`. Parentheses override.

    query   := or
    or      := and ("or" and)*
    and     := unary ("and" unary)*
    unary   := "not" unary | primary
    primary := "(" or ")" | predicate
    predicate :=
          "tag" ":" value      -- hierarchical prefix match
        | "text" ":" string    -- full-text in the body
        | field ":" value      -- frontmatter equality / list membership
        | value                -- bare term: shorthand for text:value
        | string               -- bare quoted phrase: shorthand for text:phrase

`value` is a bare word (letters, digits, `/`, `_`, `-`); `string` is
double-quoted. Keywords (`and`/`or`/`not`) are lexed as ordinary words and
disambiguated by position, so `field:or` is a valid predicate. A bare word or
quoted string that is not followed by `:` is shorthand for a `text:` predicate,
so `ntropy search foobar` searches bodies and `foobar and tag:work` combines
with operators.

### Predicate semantics

- `tag:programming` matches a note whose `tags` contain `programming` or any
  descendant (`programming/rust`), per the slash hierarchy (ADR 0006).
- `field:value` matches when the frontmatter scalar equals `value`, or when a
  list-valued field contains it.
- `text:"phrase"` matches when the note body matches, evaluated by the embedded
  grep engine (ADR 0011).

Comparison operators (`>`, `<`, `>=`, `<=`, for date/value ranges) are out of
v1 scope; the grammar is designed to add them as new tokens plus one predicate
branch.

## Evaluation

Stateless, single pass (ADR 0002):

1. Parse the query string into an AST (hand-written tokenizer + recursive
   descent, no parsing dependency; positioned errors).
2. Walk `all-notes/` once. For each note, parse frontmatter and keep the body
   in memory.
3. Evaluate the AST against the note: frontmatter predicates against the parsed
   YAML, `text:` predicates against the in-memory body via grep. Collect
   matches.

Frontmatter filtering and full-text run together in this one pass, so a query
mixing both (`tag:work and text:"deadline"`) costs a single scan.

## Open points

- Comparison operators and date-range queries (post-v1).

Tag matching is case-insensitive (tags are lowercase-normalized,
[ADR 0023](../adr/0023-slug-tag-and-disambiguator-normalization-rules.md));
arbitrary `field:value` predicates match the frontmatter value exactly.
