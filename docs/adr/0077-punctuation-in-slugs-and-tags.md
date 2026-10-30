# 77. Punctuation in slugs and tags

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0023](0023-slug-tag-and-disambiguator-normalization-rules.md),
whose step 4 removes every character outside `[a-z0-9-]` from a slug and from
a tag segment, and [ADR 0009](0009-generic-group-by-field-view-definitions.md),
which gives grouping values the normalization of tags.

## Context

Step 4 of the slug rule dropped punctuation. `Node.js notes` became
`nodejs-notes`, and `a&b` became `ab`.
Words fused, and titles that differed only in punctuation shared a slug.

Tag segments ran through the same step, so a tag was rewritten without notice:
`milestone/0.13` was stored as `milestone/013`, `c++` and `c#` both became `c`,
and `node.js` became `nodejs`. A bare query value could not hold a dot.

## Decision

Step 4 of the slug rule replaces every ASCII character outside `[a-z0-9-]`
with `-`, except the apostrophe, which it drops. Step 5 then collapses runs of
`-` and trims them, so `C++ vs. Rust!` stays `c-vs-rust` and `Node.js notes`
becomes `node-js-notes`. The apostrophe is dropped because step 1 already drops
the typographic apostrophe (U+2019): `Don't Panic` and `Don’t Panic` give the
same slug, `dont-panic`, and `User's Guide` stays `users-guide`.

The rule covers every slug: note filenames, the `<slug>` in view leaves and
site page names, and view name suggestions (`By_Tag` becomes `by-tag`).
Non-ASCII characters are not touched by this step. Step 1 already maps or
drops them, so `Gdańsk` is still `gdask`.

`reconcile` renames an existing note whose filename slug no longer matches its
title, once. Links in other notes resolve by ULID and are relinked.

### Tags

A tag segment keeps its punctuation. It runs through steps 1 to 3, 5 and 6 of
ADR 0023 and step 4 keeps every printable ASCII character except `/`: `0.13`,
`c++` and `node.js` are stored, listed and matched as written. Control
characters are dropped, and non-ASCII characters are handled as before (mapped
by step 1 or dropped). A tag matches case-insensitively, as before. A grouping
value of any view field is normalized the same way.

### Paths

A tag or grouping value becomes a view directory and a site page path through
its path form: the slug of each `/`-separated segment, with segments that
come out empty dropped. `milestone/0.13` is `milestone/0-13`. The path form
holds only `[a-z0-9-]` segments, so a path never contains `.` or `..`, a
leading dot, a character that is invalid on a filesystem or one that is
special in a URL.

Values with equal path forms (`node.js` and `node-js`, or `c++` and `c#`)
share one view directory and one site page. The directory lists the notes of
all of them, a note carrying several of them appears once, and the leaf names
inside it stay unique by ADR 0068. The site page names every member value in
its heading and sidebar label, sorted and joined with `, `, and its search
entry lists them all.

## Consequences

- A title that contains an ASCII character outside `[a-z0-9-]` other than the
  apostrophe gets a new filename on the first `reconcile`, `render`
  defaults to the new name (`./<slug>.pdf`), and the site's note page URL
  `notes/<slug>.html` changes with it.
- The first `reconcile` after the change replaces view directories named by
  the old rule (`by-tag/milestone/013` becomes `by-tag/milestone/0-13`), and
  site tag page URLs change the same way. A `tag:` query, a site `root` or a
  navigation `tag` written in the old stripped form no longer matches.
- Tags that differ only in punctuation stay distinct in `tags`, in queries and
  in the notes themselves. Only the derived view directories and site pages
  merge them.
- A tag or value without any path segment (`...`, `+`) is stored and
  queryable, but has no view directory and no site page. Its chip on a note
  page is not a link.
- A tag made only of characters that step 1 drops (`日本語`) gives no tag.
- Tags may contain a comma, which the comma-joined tags column of the plain
  note table does not distinguish from two tags.
- The language server inserts a tag that would not read back as the same
  string from a YAML list (`0.13`, `true`, `a,b`) as a double-quoted string.
