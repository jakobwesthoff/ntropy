# Non-string `title:` in frontmatter is reported as "no `title` field"

## Severity

Papercut / misleading diagnostics. No crash, no data loss.

## Problem

`src/note/frontmatter.rs:122-127` extracts the title with:

```rust
let title = mapping
    .get(Value::from("title"))
    .and_then(Value::as_str)
    ...
    .ok_or(FrontmatterError::MissingTitle)?;
```

`Value::as_str` returns `None` for any non-string YAML scalar. YAML resolves
unquoted `2026`, `3.14`, `true`, `null` to typed scalars, so a note with

```yaml
title: 2026
```

is rejected with `the frontmatter has no `title` field` — but the user
plainly wrote a title. The same silent-drop happens for tags:
`extract_tags` (`src/note/frontmatter.rs:143-151`) uses
`filter_map(Value::as_str)`, so `tags: [2026, rust]` silently loses `2026`.

## Options

1. Coerce scalar titles/tags to their string rendering (`2026` → "2026").
   Most forgiving; matches the permissive-frontmatter spirit of ADR 0005.
2. Keep the strict behavior but add a distinct error variant, e.g.
   ``the `title` field is not a string (found a number)``, so the user
   can fix the file. Same for a per-entry warning on dropped tags.

Either way the current message misdiagnoses the file. Decide direction
(coerce vs. better error) before implementing; ADR 0005 documents the
permissive schema and may need a one-line amendment.

## Where

- Title: `src/note/frontmatter.rs:122-127`
- Tags: `src/note/frontmatter.rs:143-151`
