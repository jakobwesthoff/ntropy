# 95. Title and tags reach a Typst theme as text

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0081](0081-numbers-and-booleans-in-titles-and-tags-read-as-text.md),
whose last Decision paragraph says the note's raw frontmatter mapping keeps
the parsed number for rendering.

## Context

The Typst document passes the whole frontmatter to the theme's `note` as a
typed Typst value, so `tags: [2026, true]` reached it as an integer and a
boolean. The built-in theme draws each tag with `text(...)`, which takes
content or a string, so rendering such a note to PDF failed with "expected
content, found integer". A float tag and a boolean tag fail the same way.
`--to html` was not affected.

## Decision

The Typst document, in both the `typst` and the `pdf` format, passes `title`
and `tags` to the theme as ADR 0081 reads them:

- A string, number or boolean in `title` is its text. A number is spelled as
  the parser's display of it.
- Each entry of `tags` is its text the same way, and a lone scalar `tags`
  field is a string.
- A `null` entry of `tags` is dropped.
- A YAML tag such as `!label` is looked through, on an entry and on the
  whole `tags` field.

Every other field reaches the theme as the typed value it parsed to. The
prepared document's frontmatter mapping stays the parsed mapping, and field
queries, view grouping, the HTML and site exports and the site's search data
read it unchanged.

## Consequences

A theme that read `frontmatter.title` or an entry of `frontmatter.tags` as a
number or a boolean receives a string.
