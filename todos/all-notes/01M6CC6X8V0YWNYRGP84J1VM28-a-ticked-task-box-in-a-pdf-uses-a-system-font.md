---
title: "A ticked task box in a PDF uses a system font"
kind: bug
component: render
origin: review
---
# A ticked task box in a PDF uses a system font

The built-in PDF look is meant to depend only on the fonts the `typst`
binary bundles, so a note renders the same on every machine
(`src/render/typst/prelude.typ:11-13` says so). The tick in a done task
box breaks that: none of the bundled text fonts has the glyph, and typst
silently takes it from a system font. Found on 2026-11-01 in the review of
the theme test material, reproduced with ntropy 2.9.0.

## Problem

`prelude.typ:88` draws the tick as `text(size: 0.75em, fill: luma(40),
sym.checkmark)`. For a note with `- [x] done`, `pdffonts` on the PDF lists

```
KQAHNE+LibertinusSerif-Bold-Identity-H   CID Type 0C   ...
ACDNSL+LibertinusSerif-Regular-Identity-H CID Type 0C  ...
DJRJAV+.SFNS-Regular                     CID TrueType  ...
```

`.SFNS-Regular` is the macOS system font. With `--ignore-system-fonts` typst
takes New Computer Modern Math instead. There is no warning, under
`--strict` either. The `document-themes` note describes this fallback as
the expected behaviour.

## Suggested fix

Draw the tick from a bundled font named explicitly, or as a shape, so the
box looks the same everywhere, and add a test that the default theme's
PDF embeds only bundled fonts. Correct the `document-themes` note.
