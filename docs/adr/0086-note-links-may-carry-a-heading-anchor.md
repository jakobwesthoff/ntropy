# 86. Note links may carry a heading anchor

Date: 2026-11-01

## Status

Accepted

Extends [ADR 0028](0028-note-to-note-links-as-standard-markdown-links.md),
whose link target ends in `.md`, and changes the label that
[ADR 0044](0044-cross-document-links-between-rendered-notes.md) gives a
resolved note link.

## Context

A link to a heading in another note, `<ulid>-<slug>.md#<heading-id>`, did
not end in `.md`, so it was not a note link. The exports treated it as a
file reference and looked for a vault file with `#<heading-id>` in its
name, which failed with a "referenced file not found" warning.

## Decision

A note link target is a note file name, as in ADR 0028, optionally followed
by `#` and an anchor: `[display](<ulid>-<slug>.md#<heading-id>)`.

- The target is split at its first `#`. When the part before it is a note
  file name, the rest is the anchor. Otherwise the whole target is tried as
  a file name, so a target such as `<ulid>-a#b.md` stays a note link without
  an anchor. A query string does not make a note link.
- `reconcile` refreshes the file-name part and leaves everything from the
  `#` on as written.
- The anchor is the id of a heading on the target's page. Exported heading
  ids follow the rule of the HTML engine: lowercase, spaces to hyphens,
  punctuation dropped, repeated ids numbered `-1`, `-2`.
- A resolved link with an anchor shows the text the author wrote, where a
  link without an anchor shows the target's title, since the title would
  hide which section the link names. A blank text falls back to the title.
  This holds for the HTML and the Typst output alike.
- HTML output: the href is the target's page followed by `#` and the anchor
  as written. The PDF has no addressable headings, so the Typst output
  links to the target's artifact and drops the anchor.
- `site` checks every anchor against the heading ids of the target's page.
  The anchor is compared after percent-decoding, exactly and
  case-sensitively; `top` is always valid, as it is for a browser. An anchor
  that matches no heading is a warning. Ids set by raw HTML in the target
  are not seen. `render --to html` writes one note and does not check.
- The language server and `reconcile` treat the link as a link to the note.
