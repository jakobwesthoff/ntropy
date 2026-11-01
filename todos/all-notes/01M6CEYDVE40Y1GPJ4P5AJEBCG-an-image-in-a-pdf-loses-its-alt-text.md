---
title: "An image in a PDF loses its alt text"
kind: bug
component: render
origin: discussion
---
# An image in a PDF loses its alt text

A local Markdown image `![A diagram of the flow](flow.svg)` becomes
`#image("...")` in Typst without its alt text, so the PDF carries no
alternative text and a screen reader announces nothing for it.

## Problem

`src/render/typst/emitter.rs`, `image` (around line 227 on f0643bd):
the local branch writes `#image("path");` and ignores `alt`; only the
remote branch uses it, as the link label. Read in the code (2026-11-01).

## Decision

The user decided on 2026-11-01 (AskUserQuestion in the lead session of
the todo run): pass the alt text to the PDF. Nothing visible changes.

## Suggested fix

Write `#image("path", alt: "...")` when the alt text is not empty, as a
Typst string literal. Check that typst 0.15.1 writes it into the PDF
(the tagged structure or the `/Alt` entry) with a real-typst test, and
update `docs/design/typst-engine.md` and the CHANGELOG.
