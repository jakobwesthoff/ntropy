---
title: "The header of an HTML document cuts its title off on a phone"
kind: bug
component: render
origin: review
tags: [ux]
---
# The header of an HTML document cuts its title off on a phone

In the website theme, a page from `ntropy render --to html` puts the note's
title into the header bar. At 390 px a long title is cut off mid-word with
no ellipsis: "Exporting a website" shows as "$ Exporting a we". Seen on
2026-11-01 in the screenshots of the narrow-screen table work
(`document-website-390-dark.png`), on the binary of that branch.

## Problem

`.site-name` is `inline-flex` with `min-width: 0; overflow: hidden`, so the
title shrinks to the room left by the Docs link, the GitHub mark, search and
the colour switch, and `text-overflow: ellipsis` has no effect on an
`inline-flex` box. The site export shows the site name there, which is
short; a document shows its own title, which can be any length.

## Suggested fix

Let the title truncate with an ellipsis (an inner element that is `block`
or `inline-block` with `text-overflow: ellipsis` and `white-space: nowrap`),
or give a document's header the same narrow-screen treatment the site name
got. Check both themes at 320 and 390 px.
