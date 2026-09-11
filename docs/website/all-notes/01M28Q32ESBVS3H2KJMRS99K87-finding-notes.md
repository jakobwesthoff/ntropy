---
title: Finding notes
tags: [docs/find]
site:
  index: true
  label: Finding notes
  order: 3
---
`search` (alias `list`) is the one entry point for browsing, filtering,
full-text search, and opening notes. Run it with no query at all to list
the whole vault, narrow it with a query, or hand it a full ULID to jump
straight to a specific note. A single match opens the note directly;
several matches open an interactive picker to choose from.

Two pages cover how that works: [the query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md)
for the `tag:`, `field:`, and `text:` terms `search` accepts, and
[the interactive picker](01M28Q32FZCNHV2PZ5FSG8E360-the-interactive-picker.md)
for how it filters and how to move around in it.

For scripts, `-p` prints the matched note's path instead of opening it,
`-P` prints the note's content, and `-n` turns off the picker and the
editor entirely so nothing waits on a terminal. See
[Scripting and the shell](01M28Q32N56MS70ETPES4794HJ-scripting-and-the-shell.md)
for the details.
