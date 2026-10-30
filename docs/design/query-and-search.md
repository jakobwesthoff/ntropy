# Query and search

How ntropy filters and searches notes. Consolidates
[ADR 0002](../adr/0002-stateless-filesystem-scanning-over-a-derived-index.md),
[ADR 0005](../adr/0005-permissive-frontmatter-schema-with-recognized-fields.md),
[ADR 0006](../adr/0006-hierarchical-tags-by-slash-convention.md),
[ADR 0030](../adr/0030-replace-ripgrep-stack-with-regex-crate-for-full-text-search.md),
[ADR 0012](../adr/0012-query-dsl-with-hand-rolled-parser.md), and
[ADR 0076](../adr/0076-adjacent-terms-in-a-query-are-an-implicit-and.md).

## One mechanism

Filtering and full-text search are the same thing: a query expression. There
is no separate "filter" versus "search" path. Full-text is just the `text:`
predicate. A `search` command, if provided, is sugar for a bare `text:` query.

## v1 grammar

Precedence: `not` > `and` > `or`. Parentheses override.

    query   := or
    or      := and ("or" and)*
    and     := unary (["and"] unary)*
    unary   := "not" unary | primary
    primary := "(" or ")" | predicate
    predicate :=
          "tag" ":" (value | string)   -- segment sub-path match
        | "text" ":" (value | string)  -- full-text regex over the body
        | field ":" (value | string)   -- frontmatter equality / list membership
        | value                        -- bare term: shorthand for text:value
        | string                       -- bare quoted phrase: shorthand for text:phrase

`value` is a bare word (Unicode letters, digits, `/`, `_`, `-`); `string` is
double-quoted and may contain spaces and regex metacharacters (with `\"` and
`\\` escapes). Any predicate value may be either form, so a multi-word value is
written quoted (`status:"in progress"`). Keywords (`and`/`or`/`not`) are lexed
as ordinary words and disambiguated by position, so `field:or` is a valid
predicate. A bare word or quoted string that is not followed by `:` is
shorthand for a `text:` predicate, so `ntropy search foobar` searches bodies and
`foobar and tag:work` combines with operators.

Two operands side by side are an `and` that binds like the written one (ADR
0076): `borrow checker` is `borrow and checker`, `a b or c` is
`(a and b) or c`, `(a) b` is `(a) and b`, and `not a b` is `(not a) and b`. A
keyword where an operand is expected is a search word, so `and b` and
`a and and b` search bodies for `and`.

On the command line each trailing argument is read on its own (ADR 0076). An
argument with whitespace and no query syntax (no `:`, parenthesis or `"`, and
none of the words `and`, `or`, `not` in any letter case) is a phrase: its words
in order with any whitespace, a line break included, between them, and every
other character literal. So `ntropy search "borrow checker"` is a phrase, and
`ntropy search 'tag:work and not status:done'` stays a query. Beside other
arguments, an argument that is a complete query is wrapped in parentheses, so
`search 'a or b' c` is `(a or b) and c`; a piece of a query split at a space
joins as it is. Blank arguments are dropped. A phrase that contains query
syntax is a quoted string inside the query, `ntropy search '"page not found"'`;
that quoted form is a plain regex and does not match across a line break.

### Predicate semantics

- `tag:Q` is a **segment sub-path match**: `Q` and each note tag are split on
  `/`, and `Q` matches when its segment list appears as a contiguous run of full
  segments anywhere within a note tag's segments. So `tag:programming` matches
  `programming`, `programming/rust`, `area/programming` and
  `area/programming/cli`; `tag:programming/rust` matches any tag containing that
  consecutive chain. Segments are normalized (ADR 0023, ADR 0077): lowercased,
  transliterated, whitespace turned into `-`, punctuation kept. The match is
  case-insensitive (ADR 0006), and `tag:milestone/0.13` matches
  `milestone/0.13` but not `milestone/013`.
- `field:value` matches when the frontmatter scalar equals `value`, or when a
  list-valued field contains it (exact, case-sensitive).
- `text:` (and the bare-term shorthand) is a **regex** evaluated by the `regex`
  crate over the note body (ADR 0030), compiled with **smart-case**:
  case-insensitive unless the pattern contains a literal uppercase character. An
  invalid regex is a query error.

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
   YAML, `text:` predicates against the in-memory body via the `regex` crate.
   Collect matches.

Frontmatter filtering and full-text run together in this one pass, so a query
mixing both (`tag:work and text:"deadline"`) costs a single scan.

## Open points

- Comparison operators and date-range queries (post-v1).

Tag matching is case-insensitive (tags are lowercase-normalized,
[ADR 0023](../adr/0023-slug-tag-and-disambiguator-normalization-rules.md)) and
keeps punctuation ([ADR 0077](../adr/0077-punctuation-in-slugs-and-tags.md));
arbitrary `field:value` predicates match the frontmatter value exactly.
