---
title: "Code masking misses multiline code spans and cites a missing limitation doc"
kind: bug
component: note
origin: review
---
# Code masking misses multiline code spans and cites a missing limitation doc

Found in the 2026-07-02 codebase review of link extraction. `src/link/code.rs` works out which byte ranges of a note body are Markdown code, so that link extraction and the `reconcile` body rewrite skip links that are only quoted as code. Two gaps: inline code spans that cross a line break are not masked, and the module comment points to a documented limitation that no document contains.

## Problem

### Multiline inline spans

`mask_inline_spans` (`src/link/code.rs`, around line 131) works line by line, and its doc says "Spans confined to one line are handled". CommonMark inline code spans may cross line boundaries, with the newline rendered as a space. Established by reading the per-line masking logic during the review:

```markdown
before `start of a code span
[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-stale.md) ends here` after
```

This renders as one code span. The opening backtick on line 1 has no closer on its own line, so it stays literal, and the link on line 2 is not masked. The link is therefore extracted, and `rewrite_body` in `reconcile` rewrites its target inside rendered code when the slug is stale.

### Missing limitation document

The module doc (`src/link/code.rs`, around lines 12 to 14) says indented code blocks are deliberately not masked, "matching the documented limitation that links in indented code are still real links". A search of `docs/` (ADR 0028 and the design docs) finds no such limitation. Either the document was never written or it lived in a discarded plan file.

## Impact

Only ntropy-shaped targets (`<ULID>[-slug].md`) inside a multiline span are affected, and the rewrite keeps the note identity. Still, a stale link inside quoted example text is silently edited.

## Suggested fix

Mask inline code spans that cross a line break, with a small state machine that carries an open backtick run across lines. Then write the remaining limitation (indented code blocks are not masked) into ADR 0028 or the relevant design doc, and point the `code.rs` module comment at it.

## Decisions

- 2026-10-30, user: mask inline code spans that cross a line break. The option to document the single-line scope as a limitation is dropped.

## Done when

- A test pins that a multiline inline span containing an ntropy link is masked.
- The limitation is documented where maintainers can find it, and the module doc's claim matches an existing document.
