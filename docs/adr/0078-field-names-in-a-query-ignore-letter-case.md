# 78. Field names in a query ignore letter case

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0012](0012-query-dsl-with-hand-rolled-parser.md), whose
`field:value` predicate looks the field up by its exact name.

## Context

The keywords `tag:` and `text:` match in any letter case, so `Tag:work` is a
tag predicate. A field predicate looked its name up in the frontmatter
mapping letter for letter, so `Status:done` matched nothing against a note
with a `status` key, and the query returned an empty result without an error.

A frontmatter field named `tag` or `text`, in any letter case, cannot be
queried, as the parser reads that word before the colon as the keyword.

## Decision

A field predicate finds its field by name with letter case folded. The fold
is Unicode lowercase, in the CLI and in the site's search alike, so `größe:x`
and `GRÖßE:x` find a `Größe` key. The keywords `tag`, `text`, `and`, `or` and
`not` stay case-insensitive for ASCII letters only.

A note can hold keys that differ only in case. A field predicate on that name
matches when any of those keys holds a matching value, so a note with
`Status: open` and `status: done` matches both `status:open` and
`status:done`, whatever the order of the keys.

Values do not change. The CLI compares a value exactly and with case, so
`status:Done` does not match `status: done`. The site's search keeps its own
reading of a value, a case-insensitive substring
([ADR 0052](0052-client-side-search-as-a-typescript-query-dsl.md), amended).

`tag` and `text` are reserved names in every letter case. The grammar has no
escape for them, so a frontmatter field with such a name is not reachable
through a field predicate.

The fold covers query predicates only. A view groups by the exact field name.
