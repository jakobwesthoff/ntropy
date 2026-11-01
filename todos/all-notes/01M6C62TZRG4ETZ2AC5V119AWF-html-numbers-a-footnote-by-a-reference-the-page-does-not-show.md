---
title: "HTML numbers a footnote by a reference the page does not show"
kind: bug
component: render
origin: review
---
# HTML numbers a footnote by a reference the page does not show

In `site` and `render --to html`, a footnote that is first referenced
inside a definition the page does not draw takes its number from that
hidden reference. For

```markdown
[^z]: sees[^b]

x[^c] y[^b]

[^b]: B

[^c]: C
```

HTML shows `x2 y1` with the notes `1 B`, `2 C`. `render --to ansi` (patine
2.2.2) and Typst show `x1 y2` with `1 C`, `2 B`, which is the rule the
HTML engine documents: numbered in the order of the first references the
page shows. Found on 2026-11-01 when comparing the formats for the patine
2.2.2 update; reproduced with the binary built from 235fae7.

## Problem

`Walker::finish` in `src/render/markdown/mod.rs` (around line 614) walks
`footnote_references` in the order the walk met them and skips only labels
that are not reached. The walk records every reference, also those inside
the definition of `[^z]` that nothing reaches, so `[^b]` comes before
`[^c]`. The HTML emitter numbers footnotes in the order `finish` hands them
over (`src/render/html/emitter.rs`, `footnote`).

## Suggested fix

Order the drawn footnotes by their first reference that the page shows
(the body, or a drawn definition's text or headings), not by the first
reference anywhere. Check `docs/design/html-engine.md` (Footnote row) and
the CHANGELOG entry of 282d125 against the result.
