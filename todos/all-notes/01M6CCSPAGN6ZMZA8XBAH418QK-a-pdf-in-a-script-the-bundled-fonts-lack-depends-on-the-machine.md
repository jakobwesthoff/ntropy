---
title: "A PDF in a script the bundled fonts lack depends on the machine"
kind: bug
component: render
origin: review
---
# A PDF in a script the bundled fonts lack depends on the machine

The built-in PDF look promises the same result on every machine because it
uses only the fonts the `typst` binary bundles
(`src/render/typst/prelude.typ:11-13`). That holds only for the scripts
those fonts cover. Found on 2026-11-01 in a plan review, reproduced with
ntropy 2.9.0.

## Problem

A note with `Latin, Ελληνικά, Кириллица, 日本語, العربية, עברית.` renders on
macOS with these embedded fonts (`pdffonts`):

```
LibertinusSerif-Bold-Identity-H
LibertinusSerif-Regular-Identity-H
STSong
Muna
```

Greek and Cyrillic come from Libertinus Serif. Japanese and Arabic come from
macOS system fonts, which typst picks silently. With
`--ignore-system-fonts` the same characters become empty boxes, with no
warning, under `--strict` either. On a machine without such fonts the
reader gets the boxes.

## Suggested fix

Falling back to a system font is better than empty boxes, so the render
should keep doing it. The promise is what is wrong: correct the prelude
comment and the `document-themes` and `rendering-to-pdf-and-html` notes
to say which scripts the built-in look covers on its own and that other
scripts use the machine's fonts. Show how a theme names a font for such a
script so its PDFs look the same everywhere.
