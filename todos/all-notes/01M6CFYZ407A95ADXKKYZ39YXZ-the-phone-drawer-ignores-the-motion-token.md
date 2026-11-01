---
title: "The phone drawer ignores the motion token"
kind: bug
component: site
impact: low
origin: review
---
# The phone drawer ignores the motion token

The site-themes note says `--motion` is the length of every transition,
but the sidebar drawer on a phone slides with a fixed `0.2s ease-out`, so
a theme that changes `--motion` does not change the drawer.

## Problem

`src/site/theme/style.css:1588` and
`docs/website/.ntropy/themes/site/ntropy/style.css:1552` (on 91905f1),
inside the narrow-screen rules: `transition: transform 0.2s ease-out;`.
Every other transition in both stylesheets uses `var(--motion)`. Found by
the docs consolidation's fact check on 2026-11-01 (`grep -n transition`).

## Suggested fix

Use `var(--motion)` there, keeping the easing, in both stylesheets; the
reduced-motion rule that sets `transition: none` stays. A static CSS test
like the print-palette test in `src/site/theme.rs` pins that every
`transition` uses the token.
