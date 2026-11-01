# 88. Links to a note's own headings resolve in its PDF

Date: 2026-11-01

## Status

Accepted

Amends [ADR 0086](0086-note-links-may-carry-a-heading-anchor.md) and
[ADR 0044](0044-cross-document-links-between-rendered-notes.md): within one
note's PDF a link to a heading of that note no longer drops the anchor or
opens the artifact at its first page.

## Context

A PDF of one note carried no addressable headings, so a link to a heading
of the note itself was a dead URL (`[back](#lists)`) or a link to the note's
own artifact (`[back](<ulid>-<slug>.md#lists)`, the form of ADR 0086).
Typst can address a heading through a label, and a label that does not
exist fails the compile.

## Decision

In the Typst output, a link to a heading of the note being rendered links
to that heading inside the document. This covers a link that is only a
fragment and a note link to the note itself that carries a heading anchor.

- Every heading ends in a label `<ntropy-h-N>`, N the number of the heading
  in the order the output draws them. The label carries the number and not
  the heading id, since an id keeps characters, `²` among them, that Typst
  rejects in a label. The prefix `ntropy-h-` belongs to ntropy.
- The fragment is percent-decoded and compared exactly and
  case-sensitively with the ids the HTML output gives the headings of the
  same note, repeated-text suffixes and the reserved footnote ids included.
- A fragment that names a heading links to its label. A fragment that
  names none shows the link's text and raises a warning, so `--strict`
  fails. `#top`, and an empty fragment on a fragment-only link, with no
  heading of that id link to the top of the first page.
- A note link to the note itself keeps its `notelink` styling. One without
  an anchor, or with an empty anchor, still links to the note's artifact.
- A note link with an anchor to another note still drops the anchor and
  links to that note's artifact, as ADR 0086 decided, since the heading is
  in the other note's PDF.
- The HTML output is unchanged: a fragment is left to the page, where it
  may name an `id` the note's raw HTML defines.
