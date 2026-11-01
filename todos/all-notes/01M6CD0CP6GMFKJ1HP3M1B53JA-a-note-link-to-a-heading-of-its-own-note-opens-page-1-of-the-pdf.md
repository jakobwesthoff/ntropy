---
title: "A note link to a heading of its own note opens page 1 of the PDF"
kind: bug
component: render
origin: review
---
# A note link to a heading of its own note opens page 1 of the PDF

A note link with a heading anchor that points at the note itself, such as
`[back](01M6...-self-link.md#lists)` (the form of ADR 0086), jumps to the
heading in HTML. In a PDF it opens the note's own file at page 1. Found on
2026-11-01 in the review of the fix for todo 01M6CBSBHYKG206QF8BP3W9GH6,
reproduced with ntropy 2.9.0.

## Problem

For a note "Self link" with `## Lists` and that link,
`ntropy render --to typst` writes

```typst
#link("self-link.pdf")[#notelink[back]]
```

with no warning. `note_link` in `src/render/typst/emitter.rs` drops the
anchor, as ADR 0086 decided for links to other notes, since a PDF of one
note cannot address headings of another. `emit` does not receive the
note's own id, so it cannot tell a link to itself from one to another
note; `engine.rs` has `doc.id`.

## Suggested fix

Once the anchor-only links of todo 01M6CBSBHYKG206QF8BP3W9GH6 resolve to
heading labels, treat a note link to the note itself with an anchor the
same way: an internal link to the heading's label, or plain text with a
warning when the anchor names no heading. A note link to the note itself
without an anchor keeps today's behaviour. ADR 0086 says the PDF has no
addressable headings, so the change gets a new ADR that amends it.
