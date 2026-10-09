---
title: "A non-string title is reported as a missing title field"
kind: bug
component: note
status: needs-discussion
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

## Options

1. Coerce scalar titles and tags to their string rendering, so `2026`
   becomes "2026". This is the most forgiving and fits the permissive
   frontmatter of ADR 0005.
2. Keep the strict behaviour but add a distinct error variant, such as
   ``the `title` field is not a string (found a number)``, so the user can
   fix the file, plus a per-entry warning for dropped tags.

Either way the current message misdiagnoses the file. The direction,
coercion or a better error, has to be settled before implementing. ADR
0005 documents the permissive schema and may need a one-line amendment.
