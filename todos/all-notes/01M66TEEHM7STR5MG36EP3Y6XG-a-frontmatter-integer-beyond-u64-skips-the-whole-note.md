---
title: "A frontmatter integer beyond u64 skips the whole note"
kind: bug
component: note
impact: low
origin: review
tags: [error-handling]
---
# A frontmatter integer beyond u64 skips the whole note

A note whose frontmatter holds an integer larger than `u64::MAX`, in any field, is skipped with "the frontmatter is not valid YAML", although the YAML is valid and the field is not one ntropy reads. Found by the case-insensitive field lookup work on 2026-10-30 and reproduced by the lead with ntropy 2.3.0.

## Problem

Reproduction: a note with

```yaml
---
title: Big
count: 18446744073709551616
---
```

`ntropy search -n` prints `warning: skipped ...: the frontmatter is not valid YAML` and lists nothing. serde_yaml_ng fails on the value with "invalid type: integer 18446744073709551616 as u128" while building the `Value` mapping in `src/note/frontmatter.rs`, so the whole block fails to parse.

## Impact

A note with a large number, such as an ID from another system or a byte count, disappears from search and views. The warning says the YAML is invalid, which sends the user looking for a syntax error that is not there.

## Suggested fix

Find out whether serde_yaml_ng can be told to keep such integers (as a float or as a string), or catch this error and read the value as its text. At the least, name the field and the reason in the warning instead of "not valid YAML". Pin it with a frontmatter unit test and a CLI test.
