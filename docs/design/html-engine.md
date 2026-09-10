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
| Note link, resolved | a link to `<slug>.html`, in the site the collision-safe page name |
| Note link, unresolved | the display text, as in the Typst engine |
| Local image or link to a vault file | the path rewritten relative to the page; the file recorded for the export to copy |
| Code block | `<pre><code>` carrying the fence language for the highlighter ([site-frontend.md](site-frontend.md)) |
| Callout, task checkbox, note link | markup a stylesheet can address, since themes are CSS only |

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
