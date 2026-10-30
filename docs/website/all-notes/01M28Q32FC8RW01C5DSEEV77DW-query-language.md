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

# I know I wrote the phrase "borrow checker" somewhere in here.
ntropy search "borrow checker"

# A note that mentions both "borrow" and "checker", apart or in any order.
ntropy search borrow checker

# Just show me the whole pile.
ntropy search
```

## Terms

A bare word or quoted phrase that is not followed by `:` searches the
note body. Each argument you type is read on its own. Several words
inside one argument, as in `ntropy search "borrow checker"`, are a phrase:
the words in that order with any whitespace between them, a line break
included, and every character literal, so `"c++ tips"` is not a pattern.
Separate arguments, as in `ntropy search borrow checker`, find notes that
contain every word, anywhere in the body and in any order. An argument
that is a whole query, such as `'tag:work and not status:done'`, stays a
query, and a phrase that holds a `:`, a parenthesis, a quote or the word
`and`, `or` or `not` is one too. Write such a phrase with double quotes
inside single quotes: `ntropy search '"page not found"'`. For more
precise matching, reach for a typed term:

- `tag:x` matches hierarchically. `x` and each note tag are split on
  `/`, and the term matches when `x`'s segments appear as a contiguous
  run anywhere in the tag's segments. `tag:programming` therefore
  matches `programming`, `programming/rust`, and `area/programming`.
  Tag matching is case-insensitive.
- `field:value` checks frontmatter. The field's scalar value must equal
  `value` exactly, or, for a list-valued field, the list must contain
  `value`. The value match is case-sensitive, the field name is not:
  `Status:done` finds a note with a `status` field, and a note with
  both `Status: open` and `status: done` matches `status:open` and
  `status:done`. A number is compared as ntropy reads it: `x: 2.0` is
  found by `x:2.0` and not by `x:2`, and `x: 1.50` by `x:1.5`. `tag` and
  `text` always mean the terms above, in any letter case, so a frontmatter
  field named `tag` or `text` cannot be queried.
- `text:pattern` is a regex matched against the note body, compiled with
  smart case: an all-lowercase pattern matches regardless of case, but a
  pattern containing an uppercase letter only matches that case exactly.

Quote a value that contains spaces, such as `status:"in progress"`. A
dot needs no quotes, so `tag:milestone/0.13` works as it is; other
punctuation such as `+` and `#` does (`tag:"c++"`). A quoted string may
also contain regex metacharacters, and a literal quote or backslash inside
one can be written as `\"` or `\\`. A bare search word matches
as written, a dot included: `node.js` finds "node.js" and not "nodexjs". A
quoted string is a pattern, so `"node.js"` finds both.

## Combining terms

Combine terms with `and`, `or`, and `not`. `not` binds tighter than
`and`, which binds tighter than `or`, so add parentheses when that
default order isn't what you mean. Two terms side by side are an `and`:
`tag:work status:open` is `tag:work and status:open`, and
`a b or c` is `(a and b) or c`.

```bash
ntropy search '(tag:work or tag:side-project) and not status:done'
```

Beside other arguments, an argument that holds a whole query is grouped:
`ntropy search 'a or b' c` is `(a or b) and c`. Blank arguments are
ignored.

A word that is not an operator is always a search word. A mistyped
operator, as in `tag:work nd status:done`, is therefore not an error but
a search for `nd`, and a doubled operator as in `a and and b` searches
for the word `and`.
