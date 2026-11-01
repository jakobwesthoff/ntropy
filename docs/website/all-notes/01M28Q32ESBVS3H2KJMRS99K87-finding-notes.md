---
title: Finding notes
tags: [docs/find]
site:
  index: true
  label: Finding notes
  order: 3
---
`ntropy search` finds a note and opens it: by a word in it, by a tag or
any other field, or by its ULID. This page covers the search, the picker
it opens, and reading a note in the terminal.

## Search

```bash
ntropy search                                # every note
ntropy search tag:work and not status:done   # the notes a query matches
ntropy search 01J8ZA2…                       # one note by its full ULID
```

`search` has the alias `list`. A single match opens in your editor, and
several open the picker. Queries combine tags, fields and words with
`and`, `or` and `not`; the
[Query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) has the
whole grammar.

## The picker

The picker is a fuzzy finder over the matches. The input line sits at
the bottom, and the results stack upward with the best match closest to
the cursor. Type to filter the list as you go. Matched characters show
in yellow and the current row in cyan, both taken from your terminal's
palette, so the picker follows your theme. The dimmed line under the
prompt counts the matches and shows the selected note's ULID. When the
terminal is wide enough, its right end names extra keys such as
`^Y path`.

| Key | Action |
|-----|--------|
| _type_ | Filter the list |
| `Down` / `Ctrl-N` | Move toward the best match |
| `Up` / `Ctrl-P` | Move toward worse matches |
| `Ctrl-D` / `Ctrl-U` | Move half a page toward the best / worse matches |
| `PageDown` / `PageUp` | Move a full page toward the best / worse matches |
| `Ctrl-W` | Delete the last word |
| `Enter` | Open the selected note |
| `Ctrl-Y` | Print the selected note's path instead, as if you had passed `-p` |
| `Esc` / `Ctrl-C` | Abort |

> [!TIP]
> The picker draws on your terminal even while the output goes into a
> pipe. `ntropy search -p | pbcopy` opens the picker and pipes only the
> chosen note's path to `pbcopy`.

Closing the editor reconciles the note you opened, the way
`ntropy reconcile` does for the whole vault: it fixes the note's file
name to match its current title and refreshes the links that point at
it.

> [!NOTE]
> A control character or bidi override in a title shows escaped, such as
> `\u{1b}`, and you match against that escaped form. Nothing in a title
> can move the cursor or redraw the screen. `Ctrl-Y` prints no path that
> holds such a character. `ntropy reconcile` gives the note its canonical
> name, which holds none.

## Read a note in the terminal

```bash
ntropy show tag:work
```

`ntropy show` prints a note in the terminal instead of opening the
editor. It takes the same selector as `search`. The frontmatter comes
first, then the styled text, at the terminal's width; `--width` sets
another. A note longer than the screen opens in your `$PAGER`, or in
`less -R` when `$PAGER` is unset. Set `NO_COLOR` to keep the styles and
lose the colours.

In a script, `-n` turns off the picker and the editor, `-p` prints a
path, and `-P` prints a note's text, so nothing waits on a terminal.
[Scripting and the shell](01M28Q32N56MS70ETPES4794HJ-scripting-and-the-shell.md)
has the details.
