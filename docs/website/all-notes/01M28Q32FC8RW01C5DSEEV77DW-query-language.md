---
title: Query language
tags: [docs/find]
site:
  order: 1
---
`search` (and `delete`) accept a small query language for narrowing which
notes to work with. The fastest way to learn it is by example:

```bash
# What was I supposed to do for work that isn't done yet?
ntropy search tag:work and not status:done

# That meeting note where somebody said "deadline"...
ntropy search text:deadline and tag:meeting

# Everything still in progress, or anything that's on fire.
ntropy search 'status:"in progress" or tag:urgent'

# I know I wrote "borrow checker" somewhere in here.
ntropy search borrow checker

# Just show me the whole pile.
ntropy search
```

## Terms

A bare word or quoted phrase that is not followed by `:` searches the
note body, so `ntropy search borrow checker` looks for that text
directly. For more precise matching, reach for a typed term:

- `tag:x` matches hierarchically. `x` and each note tag are split on
  `/`, and the term matches when `x`'s segments appear as a contiguous
  run anywhere in the tag's segments. `tag:programming` therefore
  matches `programming`, `programming/rust`, and `area/programming`.
  Tag matching is case-insensitive.
- `field:value` checks frontmatter. The field's scalar value must equal
  `value` exactly, or, for a list-valued field, the list must contain
  `value`. This match is case-sensitive.
- `text:pattern` is a regex matched against the note body, compiled with
  smart case: an all-lowercase pattern matches regardless of case, but a
  pattern containing an uppercase letter only matches that case exactly.

Quote a value that contains spaces, such as `status:"in progress"`. A
quoted string may also contain regex metacharacters, and a literal quote
or backslash inside one can be written as `\"` or `\\`.

## Combining terms

Combine terms with `and`, `or`, and `not`. `not` binds tighter than
`and`, which binds tighter than `or`, so add parentheses when that
default order isn't what you mean:

```bash
ntropy search '(tag:work or tag:side-project) and not status:done'
```
