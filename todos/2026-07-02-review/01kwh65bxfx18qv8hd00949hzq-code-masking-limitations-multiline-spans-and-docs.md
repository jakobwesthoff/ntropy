# Code masking: multiline inline spans are not masked, and the claimed "documented limitation" is documented nowhere

Found during the 2026-07-02 codebase review (unit 06, links).

## Context

`src/link/code.rs` computes which byte ranges of a note body are Markdown
code, so link extraction (`src/link/mod.rs::extract`) and the `reconcile`
body rewrite skip links that are merely quoted as code.

## Problem 1: inline code spans crossing a line boundary are not masked

`mask_inline_spans` (`src/link/code.rs:131`) operates strictly within one
line; its doc says "Spans confined to one line are handled". CommonMark
inline code spans may cross line boundaries (the newline is rendered as a
space). Consequence, verified by reading the per-line masking logic:

```markdown
before `start of a code span
[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-stale.md) ends here` after
```

renders entirely as one inline code span, but the opening backtick on line 1
finds no closer on its own line (left literal), and line 2's link is
unmasked. The link is therefore extracted, and `reconcile`'s
`rewrite_body` will rewrite the target *inside rendered code* if the slug is
stale, mutating content the author quoted verbatim.

Impact is bounded: only ntropy-shaped targets (`<ULID>[-slug].md`) inside a
multiline span are affected, and the rewrite keeps the same note identity.
Still, it silently edits quoted example text.

## Problem 2: the referenced "documented limitation" does not exist

The module doc (`src/link/code.rs:12-14`) says indented (four-space) code
blocks are deliberately not masked, "matching the documented limitation that
links in indented code are still real links". A search of `docs/` (ADR 0028,
design docs) finds no such documented limitation. Either the doc it points
to was never written or it lived in a discarded plan file.

## Suggested resolution

- Decide whether multiline inline spans should be masked (a small state
  machine carrying an open backtick run across lines would do) or whether
  the single-line scope is an accepted limitation.
- Write the actual limitation list (indented code blocks not masked; inline
  spans single-line if kept that way) into ADR 0028 or the relevant design
  doc, and fix the `code.rs` module comment to point at reality.

## Acceptance

- A test pinning the chosen behavior for a multiline inline span containing
  an ntropy link.
- The limitation is documented where users/maintainers can find it, and the
  module doc's claim matches an existing document.
