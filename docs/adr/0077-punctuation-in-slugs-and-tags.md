# 77. Punctuation in slugs and tags

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0023](0023-slug-tag-and-disambiguator-normalization-rules.md),
whose step 4 removes every character outside `[a-z0-9-]` from a slug.

## Context

Step 4 of the slug rule dropped punctuation. `Node.js notes` became
`nodejs-notes`, `Don't Panic` became `dont-panic`, and `a&b` became `ab`.
Words fused, and titles that differed only in punctuation shared a slug.

## Decision

Step 4 of the slug rule replaces every ASCII character outside `[a-z0-9-]`
with `-`. Step 5 then collapses runs of `-` and trims them, so `C++ vs. Rust!`
stays `c-vs-rust` and `Node.js notes` becomes `node-js-notes`.

The rule covers every slug: note filenames, the `<slug>` in view leaves and
site page names, and view name suggestions (`By_Tag` becomes `by-tag`).
Non-ASCII characters are not touched by this step. Step 1 already maps or
drops them, so `Gdańsk` is still `gdask`.

`reconcile` renames an existing note whose filename slug no longer matches its
title, once. Links in other notes resolve by ULID and are relinked.

## Consequences

- A title that contains an ASCII character outside `[a-z0-9-]` after
  transliteration gets a new filename on the first `reconcile`, `render`
  defaults to the new name (`./<slug>.pdf`), and the site's note page URL
  `notes/<slug>.html` changes with it.
