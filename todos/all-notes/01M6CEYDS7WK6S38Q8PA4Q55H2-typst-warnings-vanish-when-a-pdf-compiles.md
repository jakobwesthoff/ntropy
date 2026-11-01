---
title: "Typst warnings vanish when a PDF compiles"
kind: bug
component: render
origin: discussion
tags: [error-handling]
---
# Typst warnings vanish when a PDF compiles

When `typst compile` succeeds, ntropy drops everything typst wrote to
stderr, so its warnings never reach the user, and `--strict` passes. A
font a theme names that the machine lacks falls back to another font
without a word, for example.

## Problem

`src/render/typst/engine.rs`, the `Format::Pdf` arm of the engine (around
lines 115-140 on f0643bd): only a failed run (`!out.success`) reads
`out.stderr`; a successful run discards it. Read in the code
(2026-11-01); the silent font fallback was seen during the theme guide
work the same day.

## Decision

The user decided on 2026-11-01 (AskUserQuestion in the lead session of
the todo run): pass typst's warnings through as ntropy warnings, so
`--strict` fails on them like on ntropy's own warnings.

## Suggested fix

Parse typst's `warning:` blocks from stderr after a successful compile
and turn each into a `Warning`, keeping typst's message text. Drop
stderr noise that is not a warning (the branch `pdf-fonts-dir` already
drops a deprecation text on stderr of `typst query`; reuse that rule).
Tests with the stub typst for one and several warnings and for
`--strict`, plus one real-typst test with an unknown font.

## Relations

- Relates to: the wide-table warning on branch `pdf-fonts-dir`, which runs
  `typst query` after the compile in the same arm.
