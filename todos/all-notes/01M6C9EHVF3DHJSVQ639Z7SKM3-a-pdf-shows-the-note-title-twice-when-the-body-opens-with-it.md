---
title: "A PDF shows the note title twice when the body opens with it"
kind: bug
component: render
origin: review
---
# A PDF shows the note title twice when the body opens with it

`ntropy render` to `pdf` or `typst` prints the note's title twice when the
body starts with a level-one heading that reads exactly the title: once
from the theme's `note` function, which sets `title` large and centred
(`src/render/typst/prelude.typ`, around line 157), and once as the
heading. Every note created from the seed `default.md` template opens that
way. HTML output (`site` and `render --to html`) skips such a heading
because the page shows the title in its header already. Found on
2026-11-01 while auditing the document theme docs, with a render of the
v2.9.0 build.

## Problem

The shared Markdown walk skips the heading only when the output's
`Output::dropped_title` returns the title (`src/render/markdown/mod.rs`,
around line 193). The HTML output returns it; the Typst output keeps the
default `None`.

## Suggested fix

Let the Typst output return the note's title from `dropped_title`, so the
walk skips the heading as it does for HTML. A theme receives the title as
the `title` argument either way. Check `docs/design/typst-engine.md`, the
document-themes note and the CHANGELOG against the result.
