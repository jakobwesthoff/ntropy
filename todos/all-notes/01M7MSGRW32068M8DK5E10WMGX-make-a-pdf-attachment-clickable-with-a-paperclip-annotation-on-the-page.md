---
title: "Make a PDF attachment clickable with a paperclip annotation on the page"
kind: feature
component: render
horizon: someday
origin: discussion
---
# Make a PDF attachment clickable with a paperclip annotation on the page

The asset design (decided 2026-11-17,
`docs/research/asset-handling/decisions.md`, round 4) attaches a note's
non-image files to its PDF with typst's `pdf.attach` and marks the link
with a badge; readers find the file in their viewer's attachments panel.
The link itself cannot open the file. This todo holds the idea of making
it clickable.

## Context

Found on 2026-11-17 while answering whether the link opens the attachment:

- `pdf.attach` (typst 0.15.1) stores a file at document level only; it
  draws nothing on the page.
- A `/GoToE` link action can target only an embedded PDF, not other files,
  and viewer support for it is uneven.
- A FileAttachment annotation draws a paperclip icon on the page that opens
  the embedded file when clicked (Acrobat and pdf.js show it). typst
  0.15.1 does not draw one, so ntropy would add it by post-processing the
  PDF typst writes, with a PDF library, at the position of the link's
  badge.

## Open questions

- How to find the badge's position on the page after typst laid it out
  (typst's own query of the layout, or a marker the post-processing
  searches for).
- Which PDF library, and what it adds to the binary.
- Whether a newer typst draws such annotations itself, which would make the
  post-processing unnecessary.

## Revisit when

The asset layout has shipped and attachments are in use, or typst gains
file attachment annotations.
