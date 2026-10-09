---
title: "Tags lose dots and other punctuation, so version-like tags collide"
kind: bug
component: note
status: needs-discussion
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

## Options

- Keep the slug rules for view directory names, but store and match the tag
  with dots preserved (`0.13` stays `0.13` in `tags -n` and queries). Only the
  materialized path is slugified. Two tags that differ only by a dot would then
  share a view directory, which needs the view collision handling to cover tag
  directories too.
- Allow punctuation in tags altogether. Any change has to keep the view
  directories safe: a segment of `.` or `..`, a leading dot (a hidden
  directory), and characters that are invalid on some filesystems.
- Keep today's normalization and make it visible. `write`, `reconcile` or
  `--strict` warn when a tag changes under normalization, so `milestone/0.13`
  does not turn into `milestone/013` unnoticed.

## Open questions

- The query grammar's bare value (`[letters, digits, /, _, -]`,
  `docs/design/query-and-search.md`) excludes the dot, so tags with dots would
  need quoting in queries unless the bare-value character set grows as well.
- The site search (ADR 0052) and its shared query corpus
  (`tests/fixtures/query-corpus.json`) must follow whatever the CLI decides.

## Acceptance

- A decision recorded as an ADR 0023 amendment or a new ADR.
- Whatever is chosen, an author can no longer lose information from a tag
  without being told.
