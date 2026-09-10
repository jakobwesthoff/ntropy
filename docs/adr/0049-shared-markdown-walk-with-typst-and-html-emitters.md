# 49. Shared Markdown walk with Typst and HTML emitters

Date: 2026-09-11

## Status

Accepted

Restructures the emitter of
[ADR 0040](0040-custom-typst-engine-with-own-markdown-emitter.md) and adds
the converter behind the `html` format of
[ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md).
Note links keep the resolution of
[ADR 0028](0028-note-to-note-links-as-standard-markdown-links.md) and the
naming rule of [ADR 0044](0044-cross-document-links-between-rendered-notes.md).

## Context

The Typst emitter walks pulldown-cmark's event stream with a stack of
frames, one per open container, and formats Typst at every closing
handler. Its structural mechanisms are independent of Typst: note links
are classified by matching an event's byte span against the resolved link
table, bare URLs are found with `linkify`, footnotes are buffered and
patched in a second pass, image alt text is flattened, callout kinds are
extracted. The code fuses those mechanisms with Typst output; there is no
output abstraction.

HTML output needs the same mechanisms plus things neither engine has:
heading ids for anchors and an outline, asset paths rewritten relative to
the page with the referenced files recorded for copying, and markup a
stylesheet can address for callouts, task checkboxes, and note links.

pulldown-cmark's own HTML renderer (behind the crate's `html` feature,
which ntropy builds without) emits `<a>` with `href` and `title` only,
`<blockquote class="markdown-alert-<kind>">` without a title element, and
`<pre><code class="language-x">`; anything beyond that has to be injected
as raw HTML events. comrak is a second CommonMark parser with its own AST
and extensions.

## Decision

One structural Markdown walk over pulldown-cmark behind an output trait,
with the existing Typst emitter and a new HTML emitter as its two
implementations. Built in three stages:

1. Extract the walk behind the trait as a pure refactor. The kitchen-sink
   snapshot and the escaping corpus stay unchanged.
2. Add the HTML emitter with its own writer, whose contexts are text
   content, attribute values, and raw pass-through, and its own
   kitchen-sink snapshot.
3. Add heading ids as a mechanism of the shared walk. Ids follow GitHub's
   rule: lowercase, punctuation dropped except hyphens and underscores,
   spaces become hyphens, Unicode letters stay, duplicates get `-1`, `-2`.

HTML-specific handling of constructs where the two engines differ:

- Raw HTML in a note body passes through verbatim. The Typst engine drops
  it with a warning.
- Math stays off in the parser and renders as literal text; a `mermaid`
  fence is an ordinary code block. Parity with the Typst engine.
- A resolved note link targets `<slug>.html`; the site's collision rule is
  in ADR 0054. An unresolved one degrades to its display text, as in the
  Typst engine.
- A local asset path is rewritten relative to the page, and the file is
  recorded so the export copies it.
- A code block carries its fence language for the highlighter
  (ADR 0053).

### Rejected alternatives

- **Copying the emitter's skeleton** into a second emitter without a
  shared walk, leaving the structural mechanisms duplicated.
- **pulldown-cmark's HTML renderer with an event-rewriting pass.** Less
  code, but the markup listed above is upstream's, and every construct
  beyond it becomes injected raw HTML.
- **comrak.** A second parser beside pulldown-cmark, so the PDF and the
  HTML of one note would come from different parsers.

## Consequences

- The Typst emitter is refactored; the kitchen-sink snapshot pins its
  output byte for byte, so a behavior change in stage one shows as a
  diff.
- The two engines agree on parser, options, link resolution, and warning
  policy, and diverge on raw HTML by decision.
