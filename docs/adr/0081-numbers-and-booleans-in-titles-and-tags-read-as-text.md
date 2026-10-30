# 81. Numbers and booleans in titles and tags read as text

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0005](0005-permissive-frontmatter-schema-with-recognized-fields.md),
which makes `title` required, and
[ADR 0006](0006-hierarchical-tags-by-slash-convention.md), which defines
`tags` as a list of strings.

## Context

YAML reads an unquoted `2026` as a number and an unquoted `true` as a
boolean. The note parser accepted only strings in `title` and `tags`, so
`title: 2026` was reported as "the frontmatter has no `title` field" and
`tags: [2026, rust]` lost the tag `2026` without a message.

The YAML parser does not keep the text a number was written as. `1.50`
parses to the number 1.5, `1.0` to the float 1.0, `0x1F` to 31.

## Decision

A string, number or boolean in `title` or `tags` is read as its text. A
number is spelled as the parser's display of it, so `title: 2026` is the
title "2026", `title: 1.0` is "1.0", and `tags: [2026, true]` holds the tags
`2026` and `true`. A tag read this way is then normalized like any other
tag. A value whose exact spelling matters is quoted.

Further rules for the two fields:

- `null` counts as absent: `title: null` is a missing title, and a `null`
  entry in `tags` is skipped.
- A YAML tag such as `!label foo` is looked through, so
  the value reads as `foo`.
- A list or a mapping as `title` is refused with a message that names what
  was found: "the `title` field is a list, expected a string, number or
  boolean".
- A list or a mapping as an entry of `tags` is refused with the entry's
  position: "entry 2 of the `tags` field is a list, expected a string,
  number or boolean". A mapping as the whole `tags` field is refused as well.
- A note refused this way is skipped with a warning like any other
  malformed note, and the warning is an error under `--strict`
  ([ADR 0019](0019-scan-robustness-and-resource-tolerance.md)).

Only `Note.title` and `Note.tags` are read this way. The note's raw
frontmatter mapping keeps the parsed number, so `title: 2026` stays a
number in the mapping that field queries, view grouping, rendering and
the site's search data read.

## Consequences

- A title whose parsed spelling differs from what was typed becomes a
  different title: `1.10` is "1.1", `1.50` is "1.5", `0x1F` is "31". The
  note's filename is built from the title, so `reconcile` renames a file
  named after the typed text.
- A note with a list or mapping in `title`, or in an entry of `tags`, is
  not loaded until the value is fixed. It is absent from search, views and
  `delete`, and `reconcile` does not rename or relink it.
