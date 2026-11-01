---
title: "A page printed in dark mode keeps the dark palette"
kind: bug
component: site
origin: review
---
# A page printed in dark mode keeps the dark palette

A page from `site` or `render --to html` printed while the dark palette is
in effect keeps the dark colours: in a print emulation of a demo export the
body text was rgb(231, 223, 205) on rgb(26, 21, 16). That happens with Dark
picked in the switch, or with a dark system preference and no choice made.
Found on 2026-11-01 in the review of the site theme guide, with Chrome's
print emulation over the DevTools protocol. Browsers usually drop
background colours when printing, so the likely result on paper is pale
text on white; that last part has not been tried on a printer.

## Problem

The dark palette rules in the built-in `src/site/theme/style.css` (around
lines 141 and 165) and in the documentation site's theme are not limited
to the screen. The only `@media print` block (around line 461 in the
built-in theme, 439 in the site theme) touches `.table-wrap` and
`.heading-anchor` and nothing else.

## Suggested fix

Print with the light palette whatever the scheme on screen, in both
stylesheets: limit the dark rules to `screen`, or restate the light tokens
inside `@media print`. Check with print emulation in light, dark and system
dark, and look at the PDF Chrome prints.
