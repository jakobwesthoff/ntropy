---
title: "A long link or inline code scrolls a page sideways on a phone"
kind: bug
component: site
impact: high
origin: review
---
# A long link or inline code scrolls a page sideways on a phone

On a narrow screen, a long URL shown as a link and a long inline code span
in a paragraph do not wrap, so the whole page scrolls sideways. Found on
2026-11-01 while checking the theme guide against the theme test vault,
reproduced with ntropy 2.9.0.

## Problem

Export the theme test vault
(`https://ntropy.westhoffswelt.de/downloads/theme-test-vault.tar.gz`, source
`docs/website/theme-test/`) with the built-in theme and open
`notes/theme-test-note.html` at 390 px: the page is 748 px wide. The
elements that stick out are an `a` (right edge 742) and a `code` (right
edge 679) in the note's "Wrapping" section. `src/site/theme/style.css` has
no `overflow-wrap` or `word-break` rule. The same holds for a page of
`render --to html` and for the documentation site's own theme
(`docs/website/.ntropy/themes/site/ntropy/style.css`). Tables and code
blocks scroll inside their own box and are fine.

## Suggested fix

Let running text break a word that does not fit its line, for links and
inline code included (for example `overflow-wrap: anywhere` on the body
text), in both shipped stylesheets, without breaking words that fit.
A test pins it, like the print test in `src/site/theme.rs` reads both
stylesheets. Check every page of the docs site and the theme test site
at 390 and 1280 px.
