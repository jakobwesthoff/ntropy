---
title: The interactive picker
tags: [docs/find]
site:
  order: 2
---
When `search` matches several notes it opens a fuzzy picker; a single
match skips the picker and opens the note directly, and a full ULID
jumps right to it. The picker draws on your terminal even while stdout
feeds a pipe, so `ntropy search -p | pbcopy` still runs the picker
interactively and pipes only the chosen note's path to `pbcopy`. Passing
`-n` skips the picker entirely, which is what makes `search` usable from
a script.

The layout is bottom-anchored, like a shell prompt: the input line sits
at the bottom and results stack upward, with the best match closest to
the cursor. Type to filter the list live. Matched characters glow
yellow and the current row is cyan, both colors drawn from your
terminal's own palette so the picker follows your theme.

| Key | Action |
|-----|--------|
| _type_ | Filter the list |
| `Down` / `Ctrl-N` | Move toward the best match |
| `Up` / `Ctrl-P` | Move toward worse matches |
| `Ctrl-W` | Delete the last word |
| `Ctrl-U` | Clear the query |
| `Enter` | Open the selected note |
| `Esc` / `Ctrl-C` | Abort |

Choosing a note opens it in your editor. Closing the editor reconciles
that note the same way `ntropy reconcile` does across the whole vault:
it fixes the note's filename slug to match its current title and
refreshes any links that point at it.
