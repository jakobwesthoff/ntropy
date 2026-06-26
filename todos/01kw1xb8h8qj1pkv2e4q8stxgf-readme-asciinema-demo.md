# Add an asciinema demo to the README

Record and embed an asciinema cast in `README.md`, placed directly under the
title / tagline, before "Why I built this".

## What to capture

The two features that sell ntropy and do not come across in prose:

- The bottom-anchored interactive fuzzy picker from `search` (prompt at the
  bottom, list growing upward, yellow match highlight, cyan selected row,
  live filtering).
- The language server in an editor: `[` link completion (fuzzy-matched on
  title and tags, inserting the full `[Title](<ulid>-<slug>.md)`) and
  `tags:` completion.

A short end-to-end beat would also work: `ntropy init`, `ntropy new`, then a
`search` that opens the picker.

## Mechanics

- User records the cast and uploads to asciinema.org.
- Claude wires the markup into the README (a centered linked SVG, e.g. a
  `<center><a href=...><img .../></a></center>` block, or a plain linked SVG).

## Decision context

Deferred during the 2026-06-26 README restructuring, which aligned the README to
a shared structure: title and tagline (with a "no database, no proprietary app,
no folder hierarchy" beat), a "Why I built this" narrative, Installation, Quick
Start, a body wrapped in `<!-- docs:start -->` / `<!-- docs:end -->` markers
(concepts, commands table, searching/picker, query language, views, templates,
configuration, linking/LSP, limitations), then Development, Design, and License
outside the markers. The demo slots directly under the title/tagline, ahead of
"Why I built this".

The demo was the highest-leverage optional addition, but requires a recording
the user must produce, so it was split out rather than left as a dead placeholder
in the README.
