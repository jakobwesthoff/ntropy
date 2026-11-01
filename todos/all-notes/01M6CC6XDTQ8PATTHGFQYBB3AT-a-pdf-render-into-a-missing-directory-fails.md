---
title: "A PDF render into a missing directory fails"
kind: bug
component: render
impact: low
origin: review
---
# A PDF render into a missing directory fails

`ntropy render --to pdf -o missing/dir/x.pdf` fails when `missing/dir`
does not exist, while `--to html` creates the directory and writes the
page. Found on 2026-11-01 in the review of the theme test material,
reproduced with ntropy 2.9.0.

## Problem

```
$ ntropy render --force -o missing/dir/x.pdf 'title:"Task test"'
error: while rendering the note: `typst` failed:
error: failed to write PDF file (No such file or directory (os error 2))
```

The same command with `--to html -o missing2/dir/x.html` succeeds and
creates `missing2/dir/`. The two formats should behave the same.

## Suggested fix

Create the parent directory of the output path before the PDF and Typst
renders too, as the HTML render does. Check what `--to typst` and
`--to ansi` with `-o` do and make all four formats agree.
