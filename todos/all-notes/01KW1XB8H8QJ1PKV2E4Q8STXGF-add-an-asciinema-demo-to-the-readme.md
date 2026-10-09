---
title: "Add an asciinema demo to the README"
kind: docs
component: project
origin: discussion
---
# Add an asciinema demo to the README

Deferred during the 2026-06-26 README restructuring. The demo would sit directly
under the title and tagline, before "Why I built this". It was the
highest-leverage optional addition in that restructuring. It was split out
rather than left as a dead placeholder, because it needs a recording that the
user has to produce.

## Proposal

The demo should show the two features that sell ntropy and do not come across
in prose:

- The bottom-anchored interactive fuzzy picker from `search`: the prompt at the
  bottom, the list growing upward, the yellow match highlight, the cyan selected
  row and live filtering.
- The language server in an editor: `[` link completion, fuzzy-matched on title
  and tags and inserting the full `[Title](<ulid>-<slug>.md)`, and `tags:`
  completion.

A short end-to-end sequence also works: `ntropy init`, `ntropy new`, then a
`search` that opens the picker.

The user records the cast and uploads it to asciinema.org. The markup then goes
into `README.md` as a centered linked SVG (`<center><a href=...><img .../></a></center>`)
or as a plain linked SVG.
