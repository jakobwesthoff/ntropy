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
footnote buffering, image alt flattening, callout kinds, heading ids, and
the skipping of a leading level-one heading that repeats the title an
emitter says it shows elsewhere. An emitter owns the markup and the
escaping.

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
| Heading | an `id` by GitHub's rule: lowercase, punctuation dropped except hyphens and underscores, spaces to hyphens, Unicode letters kept, duplicates suffixed `-1`, `-2`. A level-one heading that opens the body and reads exactly the note's title is skipped by the walk, before an id is spent on it: the page shows the title in its header already |
| Note link, resolved | `<a class="note-link">` showing the target's current title, its `href` decided by the caller: `<slug>.html` beside a single-note artifact, the collision-safe page name in the site |
| Note link, unresolved | the display text, as in the Typst engine |
| Local image or link to a vault file | `<img>` or `<a>` whose `src`/`href` the caller decides; the path as written is recorded so the export copies the file. Local means no URI scheme, not protocol-relative, not a fragment |
| Remote image | `<img>` with the URL as written; the browser fetches it, so nothing degrades and nothing warns |
| Code block | `<pre><code class="language-<tag>">`, the tag being the info string's first token, recorded per body for the highlighter ([site-frontend.md](site-frontend.md)) |
| Callout | `<div class="callout callout-<kind>">` with a `<p class="callout-title">` naming the kind, opening on a `<use>` of the kind's icon from the page's sprite: `info`, `lightbulb`, `message-square-warning`, `triangle-alert`, `octagon-alert` for note, tip, important, warning, caution |
| Task checkbox | a disabled `<input type="checkbox">`, checked or not |
| Footnote | a numbered `<sup class="footnote-ref">` at every reference and a `<section class="footnotes">` after the body listing each definition once with a back-link, numbered in first-reference order |

Besides the fragment, the emitter returns the local paths the body
references and the fence languages it saw, each once, so the site copies
exactly the referenced files and loads only the grammars a page needs.
Where a note link or a local path points in the artifact is not the
emitter's decision: the caller supplies it, because it depends on where the
artifact lands relative to other artifacts and to the vault.

## The `render --to html` artifact

The site's page for one note, without the parts that need a site
([ADR 0057](../adr/0057-html-artifact-as-a-page-with-a-files-directory.md)):
`<stem>.html` and, beside it, `<stem>_files/`, where `<stem>` is the
artifact's file name without its extension. The page keeps the header,
with the note's title where a site page shows the site name, the scheme
switch, and the outline; it has no sidebar, search, breadcrumbs, pager,
or related notes, its tags are plain text, and the `site` frontmatter
table is hidden and inert. The icon sprite is inlined as on a site page.
Note links target `<slug>.html` beside the artifact, the HTML
counterpart of the `<slug>.pdf` convention.

The directory mirrors the site's `assets/`
([site-export.md](site-export.md)): `style.css` and every other file of
the theme except its `icons/`, `app.js`, under `grammars/` the grammars
the page's code blocks need, and under `files/<vault path>` the images
and files the note references, resolved against the note's directory
and copied as the site copies them; a file that is missing or lies
outside the vault is a warning and stays unresolved, as does a fence
language without a grammar. The theme is selected as the site selects
it, `--theme` then `[site] theme`, resolved in `.ntropy/themes/site/`;
the `pdf` and `typst` formats keep `[render] theme` and
`.ntropy/themes/typst/`.

## Testing

The kitchen-sink fixture (`tests/fixtures/kitchen-sink.md`) is pinned
through the HTML emitter as one snapshot, as it is through the Typst
emitter.
