---
title: "Tags lose dots and other punctuation, so version-like tags collide"
kind: bug
component: note
origin: implementation
---
# Tags lose dots and other punctuation, so version-like tags collide

Tag segments are normalized with the slug rules, which drop every character
outside `a-z`, `0-9` and `-`. A dot is removed, not replaced, so tags that carry
a version or a name with punctuation are silently rewritten. The problem was
found while writing the ntropy-todos skill in the skills repository, whose
`milestone/<name>` tags suggested `milestone/0.13` as an example.

## Problem

Tag segments go through the slug segment normalizer (ADR 0023,
`src/text/tag.rs` `segments` calling `normalize_segment` in `src/text/slug.rs`).
Step 4 of that pipeline drops every character outside `[a-z0-9-]`. A dot is
removed, not replaced, so tags that carry a version or a name with punctuation
are silently rewritten:

- `milestone/0.13` becomes `milestone/013`, and `milestone/0.1.3` becomes the
  same tag
- `c++` and `c#` both become `c`
- `node.js` becomes `nodejs`

Checked with ntropy 2.1.1 on 2026-10-10 in a throwaway vault, using a note
tagged `[concurrency, milestone/0.13]`:

- `ntropy tags -n` lists `milestone/013`
- the view directory is `by-tag/milestone/013/`
- `ntropy search -n tag:milestone/0.13` is a query syntax error
  (`unexpected character .`), because a bare query value cannot contain a dot;
  the quoted form `tag:"milestone/0.13"` matches, and so does `tag:milestone/013`

Nothing warns the author that the tag they wrote is not the tag ntropy stores.
The ntropy-todos skill now restricts tag names to lowercase letters, digits,
hyphens and `/` as a workaround.

## Proposal

Tags keep their punctuation as written: they are stored and matched with it,
so `0.13` stays `0.13` in `tags -n` and in queries. Only the view directory
name is normalized, and the normalization replaces every character outside
`[a-z0-9-]` with `-` instead of dropping it. This applies to all slugs, note
filenames included, so `reconcile` renames existing notes whose titles contain
such characters once. Links resolve by ULID and are relinked. The materialized
path keeps to safe segments: no `.` or `..`, no leading dot, no characters that
are invalid on some filesystems.

## Open questions

- Whether the query grammar's bare value (`[letters, digits, /, _, -]`,
  `docs/design/query-and-search.md`) grows to include `.`, or tags with dots
  stay quoted in queries.
- Collision handling when two tags share a view directory (`node.js` and
  `node-js` both become `node-js`, `0.13` and `0-13` both become `0-13`). The view collision
  handling has to cover tag directories too.
- The site search (ADR 0052) and its shared query corpus
  (`tests/fixtures/query-corpus.json`) must follow whatever the CLI decides.

## Decisions

- 2026-10-30, user: tags keep their punctuation as written (stored and matched with it); only the view directory name is normalized.
- 2026-10-30, user: the normalization replaces every character outside `[a-z0-9-]` with `-` instead of dropping it, and this applies to all slugs, note filenames included, so `reconcile` renames existing notes whose titles contain such characters once (links resolve by ULID and are relinked). The option to keep normalization and warn is dropped.

## Acceptance

- The decision is recorded as an ADR 0023 amendment or a new ADR.
- An author can no longer lose information from a tag, and the slug change
  is covered by a `reconcile` test that renames an existing note.
