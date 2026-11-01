---
title: "Show a note in the terminal, rendered with patine"
kind: feature
component: render
origin: request
tags: [ux]
---
# Show a note in the terminal, rendered with patine

ntropy renders notes to PDF, Typst and HTML, but has no way to read a
note formatted in the terminal. patine (crates.io, same author) renders
Markdown to ANSI-styled terminal output and is a library as well as a
CLI. This adds an ANSI render engine built on patine and a command that
shows a note in the terminal.

## Decisions

- 2026-11-01, the owner asked for this integration after the lead's
  proposal, to be built now:
  - A new format in the render registry (ADR 0038), `--to ansi`, whose
    engine renders the `PreparedDocument` through `patine` (library,
    `default-features = false`): note links replaced by the target's
    title as the HTML engine resolves them, a header with title, tags
    and date, the body through `patine::render`.
  - A command `ntropy show <ID|QUERY>` that prints a note rendered this
    way to the terminal: at the terminal's width, through a pager
    (`less -R` or `$PAGER`) when it is taller than the screen, without
    colours when stdout is not a terminal or `NO_COLOR` is set, and the
    same note selection as `render` when no argument is given.
- Known cost: patine pulls in comrak, syntect and two-face, a few MB of
  binary for syntax definitions; ntropy keeps pulldown-cmark for its own
  parsing. Images show as patine's `[image: alt] (url)` text.

## Relations

- Relates to: [Add color to decorated terminal output](01KVXYS57HDZTV654Z1XX5DGE8-add-color-to-decorated-terminal-output.md)
