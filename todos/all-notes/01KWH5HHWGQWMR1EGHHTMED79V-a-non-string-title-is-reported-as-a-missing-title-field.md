---
title: "A non-string title is reported as a missing title field"
kind: bug
component: note
impact: low
origin: review
tags: [error-handling]
---
# A non-string title is reported as a missing title field

Found in the 2026-07-02 codebase review, in the note parsing code. A note
whose frontmatter says `title: 2026` is rejected with "the frontmatter has
no `title` field", although the user plainly wrote a title. Tags with
non-string values are dropped without a word. Nothing crashes and no data
is lost, but the message sends the user looking for the wrong problem.

## Problem

Read in the code during the 2026-07-02 review.
`src/note/frontmatter.rs` extracts the title (around lines 122 to 127)
with:

```rust
let title = mapping
    .get(Value::from("title"))
    .and_then(Value::as_str)
    ...
    .ok_or(FrontmatterError::MissingTitle)?;
```

`Value::as_str` returns `None` for any non-string YAML scalar. YAML
resolves unquoted `2026`, `3.14`, `true` and `null` to typed scalars, so
a note with

```yaml
title: 2026
```

fails with the missing-title error. Tags drop values the same way:
`extract_tags` in `src/note/frontmatter.rs` (around lines 143 to 151)
uses `filter_map(Value::as_str)`, so `tags: [2026, rust]` loses `2026`.

## Proposal

Coerce scalar titles and tags to their string rendering, so `title: 2026`
becomes "2026" and `tags: [2026, rust]` keeps both tags. This fits the
permissive frontmatter of ADR 0005, which may need a one-line amendment. A
title or tag that is not a scalar (a list or a mapping) still needs an error
that says what was found instead of reporting a missing field.

## Decisions

- 2026-10-30, user: coerce scalar values to strings, for titles and for tags.
