---
title: "A PDF render fails for a note with a numeric or boolean tag"
kind: bug
component: render
impact: high
origin: review
---
# A PDF render fails for a note with a numeric or boolean tag

A note whose frontmatter carries `tags: [2026, true]` cannot be rendered to
PDF, the default format of `render`; `--to html` works. ADR 0081 makes
numbers and booleans in tags read as text everywhere, so the PDF path is
the one that does not follow it. Found on 2026-11-17 by a reviewer of the
agent-skill rework, reproduced by hand the same day.

## Problem

Reproduced with the debug build of `9d53c7f` (`ntropy 2.11.0`, typst
0.15.1) on a scratch vault:

```
$ ntropy -n render -o o.pdf <note>
Rendering 2026-11-17  num tag  [2026, true]  (...)...
error: while rendering the note: `typst` failed:
error: expected content, found integer
...
341 │                   text(size: 0.95em, tag),
exit 1
```

The emitted Typst passes the frontmatter as raw Typst values,
`frontmatter: ("title": "num tag", "tags": (2026, true))`, and the
built-in theme's `note` draws each tag with `text(size: 0.95em, tag)`,
which takes content or a string, not an integer or a boolean. A year as a
tag is common, so a vault with `tags: [2026]` cannot produce a PDF of that
note at all.

## Suggested fix

Emit tags as strings in the frontmatter the emitter hands to `note`, as
ADR 0081 reads them, or draw each tag through the theme's value formatter
instead of `text(...)`. The first fixes custom document themes too. A
render test with an integer and a boolean tag pins it.
