# 78. Field predicates ignore name case and compare numbers by their spelling

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0012](0012-query-dsl-with-hand-rolled-parser.md), whose
`field:value` predicate looks the field up by its exact name;
[ADR 0052](0052-client-side-search-as-a-typescript-query-dsl.md), whose search
data carries the frontmatter as JSON; and
[ADR 0081](0081-numbers-and-booleans-in-titles-and-tags-read-as-text.md),
which says the site's search data reads a number as the parsed number.

## Context

The keywords `tag:` and `text:` match in any letter case, so `Tag:work` is a
tag predicate. A field predicate looked its name up in the frontmatter
mapping letter for letter, so `Status:done` matched nothing against a note
with a `status` key, and the query returned an empty result without an error.

A frontmatter field named `tag` or `text`, in any letter case, cannot be
queried, as the parser reads that word before the colon as the keyword.

The CLI compares a number in a field with the text the parser displays it as,
so `x: 2.0` matches `x:2.0`. The site embedded the frontmatter as JSON, where
a number is read by the browser as a double and spelled by `String`, so the
same note matched `x:2` and not `x:2.0` in the site's search. A non-finite
float (`.inf`) became `null` there and never matched.

## Decision

A field predicate finds its field by name with letter case folded. The fold
is Unicode lowercase, in the CLI and in the site's search alike, so `größe:x`
and `GRÖßE:x` find a `Größe` key. The keywords `tag`, `text`, `and`, `or` and
`not` stay case-insensitive for ASCII letters only.

A note can hold keys that differ only in case. A field predicate on that name
matches when any of those keys holds a matching value, so a note with
`Status: open` and `status: done` matches both `status:open` and
`status:done`, whatever the order of the keys.

Values do not change in the CLI: it compares a value exactly and with case,
so `status:Done` does not match `status: done`. The site's search keeps its
own reading of a value, a case-insensitive substring (ADR 0052, amended).

`tag` and `text` are reserved names in every letter case. The grammar has no
escape for them, so a frontmatter field with such a name is not reachable
through a field predicate.

The fold covers query predicates only. A view groups by the exact field name.

A number in a field is compared as the text the parser displays it as: `2.0`
stays `2.0`, `1.50` is `1.5`, `0x1F` is `31`, `1e21` stays `1e21`, `.inf` and
`.nan` keep that spelling. The site's search data carries every number as
that text instead of a JSON number, so both evaluators compare the same
string. The note's raw frontmatter mapping still holds the parsed number
(ADR 0081).

## Consequences

- A query whose field name differs in case from the key now selects notes it
  did not select before, in the CLI and in the site's search.
- In the site's search, `x:2.0` selects a note with `x: 2.0` and `x:2` no
  longer does. Non-finite floats match there too.
