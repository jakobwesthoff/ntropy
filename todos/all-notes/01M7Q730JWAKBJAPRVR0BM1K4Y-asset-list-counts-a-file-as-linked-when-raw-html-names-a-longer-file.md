---
title: "asset list counts a file as linked when raw HTML names a longer file name"
kind: bug
component: asset
impact: low
origin: review
---
# asset list counts a file as linked when raw HTML names a longer file name

When a note's raw HTML names a file in its asset folder, `asset list`
also counts every file whose name is a prefix of that name as linked.
Found on 2026-11-18 in the plan review of the `asset list` fix for
links into a folder of another name.

## Problem

`src/asset/files.rs`, `names_the_file`, matches the text after
`_assets/` with `starts_with("<folder>/<name>")`, so the end of the name
is never checked. Reproduced on 2026-11-18 with ntropy 2.14.0: a note
holding `<img src="_assets/<note>/wide.png" width="300">` with the files
`wide.png` and `wide.pn` in its folder lists both as `yes`.

## Impact

A file the note does not use reads `yes` instead of `no`, so a check
for unused files misses it.

## Suggested fix

Require the character after the name to end the reference (a quote,
whitespace, `>`, `?`, `#` or the end of the text), and add a test with a
file whose name is a prefix of the referenced one.
