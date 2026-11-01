---
title: "An HTML document keeps its outline column on a phone"
kind: bug
component: render
origin: review
tags: [ux]
---
# An HTML document keeps its outline column on a phone

A page from `ntropy render --to html` keeps its two columns, the note body
and the 13rem outline, at every width. At a 390 px viewport the body is
135 px wide and the outline beside it 221 px, so the note cannot be read.
The page does not scroll sideways, so an overflow check misses it. Found
on 2026-11-01 in the review of the narrow-screen table work, measured in
headless Chrome.

## Problem

`.document .layout { grid-template-columns: minmax(0, 1fr) var(--outline-width) }`
(specificity 0,2,0) beats the narrow-screen rule
`@media (max-width: 60rem) { .layout { grid-template-columns: minmax(0, 1fr) } }`
(0,1,0). The same pair is in the built-in theme (`src/site/theme/style.css`,
lines 635 and 1422) and the website theme
(`docs/website/.ntropy/themes/site/ntropy/style.css`, lines 613 and 1401).
It has been there since 273080c, which made `render --to html` emit the
site's page.

## Suggested fix

Let the narrow-screen rule cover documents too (`.layout, .document .layout`)
in both stylesheets, and check that the note body is the viewport width
minus the gutters at 390 px on every exported page, documents included.
