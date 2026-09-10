# The HTML engine

The converter behind the `html` format of `render` and behind every note
page of the site ([site-export.md](site-export.md)). The decision is
[ADR 0049](../adr/0049-shared-markdown-walk-with-typst-and-html-emitters.md);
the Typst side of the same walk is [typst-engine.md](typst-engine.md).

## Shape

One structural Markdown walk over pulldown-cmark, with the option set the
Typst engine uses (tables, strikethrough, task lists, footnotes, GFM
callouts; math off), drives two emitters behind an output trait: the
Typst emitter and the HTML emitter. The walk owns what is independent of
the output: the frame stack of open containers, note-link classification
by matching an event's byte span against the resolved link table of the
`PreparedDocument` ([rendering.md](rendering.md)), bare-URL detection,
footnote buffering, image alt flattening, callout kinds, and heading ids.
An emitter owns the markup and the escaping.

## Escaping

The HTML writer has three contexts, chosen at each call site as the Typst
writer's are: text content, attribute values, and raw pass-through. Every
interpolation point is one of them.

## Constructs

Where the HTML engine's handling differs from, or adds to, the Typst
engine's:

| Construct | HTML |
| :--- | :--- |
| Raw HTML in the body | passed through verbatim (the Typst engine drops it with a warning) |
| Math, `mermaid` fences | as the Typst engine: literal text, an ordinary code block |
| Heading | an `id` by GitHub's rule: lowercase, punctuation dropped except hyphens and underscores, spaces to hyphens, Unicode letters kept, duplicates suffixed `-1`, `-2` |
| Note link, resolved | `<a class="note-link">` showing the target's current title, its `href` decided by the caller: `<slug>.html` beside a single-note artifact, the collision-safe page name in the site |
| Note link, unresolved | the display text, as in the Typst engine |
| Local image or link to a vault file | `<img>` or `<a>` whose `src`/`href` the caller decides; the path as written is recorded so the export copies the file. Local means no URI scheme, not protocol-relative, not a fragment |
| Remote image | `<img>` with the URL as written; the browser fetches it, so nothing degrades and nothing warns |
| Code block | `<pre><code class="language-<tag>">`, the tag being the info string's first token, recorded per body for the highlighter ([site-frontend.md](site-frontend.md)) |
| Callout | `<div class="callout callout-<kind>">` with a `<p class="callout-title">` naming the kind |
| Task checkbox | a disabled `<input type="checkbox">`, checked or not |
| Footnote | a numbered `<sup class="footnote-ref">` at every reference and a `<section class="footnotes">` after the body listing each definition once with a back-link, numbered in first-reference order |

Besides the fragment, the emitter returns the local paths the body
references and the fence languages it saw, each once, so the site copies
exactly the referenced files and loads only the grammars a page needs.
Where a note link or a local path points in the artifact is not the
emitter's decision: the caller supplies it, because it depends on where the
artifact lands relative to other artifacts and to the vault.

## The `render --to html` artifact

One self-contained file: the site theme's stylesheet inlined, the note
content and its frontmatter block, and note links targeting `<slug>.html`
beside the artifact, the HTML counterpart of the `<slug>.pdf` convention.
No sidebar, no search. The theme is selected as the site selects it,
`--theme` then `[site] theme`, resolved in `.ntropy/themes/site/`; the
`pdf` and `typst` formats keep `[render] theme` and `.ntropy/themes/typst/`.

## Testing

The kitchen-sink fixture (`tests/fixtures/kitchen-sink.md`) is pinned
through the HTML emitter as one snapshot, as it is through the Typst
emitter.
