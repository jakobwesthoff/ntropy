---
title: "Stop inlining the full icon sprite into every site page"
kind: improvement
component: site
status: needs-discussion
origin: request
tags: [performance]
---
# Stop inlining the full icon sprite into every site page

The user asked on 2026-09-11 whether the export duplicates scripts and styles
across pages. It does not: every page links `assets/style.css`,
`assets/app.js`, the grammar scripts it needs, and loads
`assets/search-data.js` on demand, one copy each. What every page does carry
is the theme's icon sprite (ADR 0055), and a one-line inline script that
applies the remembered color scheme before paint.

Measured on an export of the user's vault on 2026-09-11: 207 pages, a sprite
of 5,685 bytes with 21 symbols per page, 1.1 MiB of sprite in total, about a
fifth of a 25 KB note page.

## Why it is inline

Chrome refuses a `<use>` of a symbol in another SVG file and a CSS mask image
over `file://` (verified during the ADR 0055 work). The site must work when
opened from disk, so the sprite cannot be one shared file that the pages
reference.

## Options

- Inline only the symbols a page uses. The page markup and the note body
  reference a known set of icon names, so the sprite per page can be that
  subset. The header alone uses about ten; a note page without callouts needs
  no callout icons.
- Split the sprite by role: the chrome's icons in every page, and the content
  icons (callouts, tags) only where the body has them.
- Inject the sprite from `app.js` at load, as one shared file, and keep the
  inline copy only for pages that must render without script. This trades a
  flash of missing icons for the duplication.

## Open questions

Whether the saving matters at the sizes above, and if so which option. The
answer has to keep `file://` working.
