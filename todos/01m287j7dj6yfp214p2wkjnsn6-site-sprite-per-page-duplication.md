# Site: the icon sprite is inlined into every page

Raised by the user on 2026-09-11 while asking whether the export
duplicates scripts and styles across pages. It does not: every page
links `assets/style.css`, `assets/app.js`, the grammar scripts it needs,
and loads `assets/search-data.js` on demand, one copy each. What every
page does carry is the theme's icon sprite (ADR 0055), and a one-line
inline script that applies the remembered color scheme before paint.

Measured on an export of the user's vault on 2026-09-11: 207 pages,
a sprite of 5,685 bytes with 21 symbols per page, 1.1 MiB of sprite in
total, about a fifth of a 25 KB note page.

## Why it is inline

Chrome refuses a `<use>` of a symbol in another SVG file and a CSS mask
image over `file://` (verified during the ADR 0055 work), and the site
must work when opened from disk, so the sprite cannot be one shared
file the pages reference.

## Options to investigate

- Inline only the symbols a page uses: the page markup and the note body
  reference a known set of icon names, so the sprite per page can be
  the subset. The header alone uses about ten; a note page without
  callouts needs no callout icons.
- Split the sprite by role: the chrome's icons in every page, the
  content icons (callouts, tags) only where the body has them.
- Inject the sprite from `app.js` at load, as one shared file, and keep
  the inline copy only for pages that must render without script; this
  trades a flash of missing icons for the duplication.

## To decide

Whether the saving matters at the sizes above, and if so which option;
the answer has to keep `file://` working.
