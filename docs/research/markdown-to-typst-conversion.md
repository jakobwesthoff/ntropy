# Markdown-to-Typst conversion: domain knowledge from pulldown_typst

Research notes distilled from reading the source of the `pulldown_typst`
crate and its sibling converter crate `pullup` (see
[Credits and license](#credits-and-license)). The goal is the problem-domain
knowledge — what a Markdown-to-Typst converter must get right — not the
crates' architecture. Facts below describe what that code does; where a
point is our inference rather than the crates' stated behavior, it is
marked as such.

The two crates split the job: `pullup` maps pulldown-cmark Markdown events
(pulldown-cmark 0.13, default features off) to a Typst event model, and
`pulldown_typst` serializes those Typst events to markup text. The
knowledge below covers both halves, since a fresh implementation needs
both.

## Escaping

The single most important fact: Typst output has **two distinct escaping
contexts**, and the crates handle them with two separate escape functions.

### Markup context (plain text, headings, list items, table cells, link labels, quote bodies)

Ordinary text is escaped by backslash-prefixing each of:

    $  #  <  >  *  _  `  @

- `$` opens math, `#` opens code mode, `` ` `` opens raw, `*`/`_` toggle
  strong/emphasis, `<`/`>` delimit labels, `@` starts a reference.
- Quirk: underscore is replaced with `` \_`` *including a leading space*,
  so `_whatever_` in source text becomes `` \_whatever \_`` in output. The
  crate's own test pins this space-inserting behavior; it visibly alters
  the text and reads as a workaround, not a design.
- This escape is applied to text everywhere outside code: paragraph text,
  heading text, table cell content, link label content, quote bodies.
- **Not escaped**: backslash itself, `[`, `]`, `/`, `=`, `-`, `+`, `~`,
  `'`, `"`. Several of these are live Typst syntax. A literal `\` in
  Markdown text passes through and Typst reads it as an escape sequence;
  an unbalanced `]` in text emitted inside a `#emph[...]`, `#link(...)[...]`,
  or table-cell `[...]` content block terminates that block early; `//`
  starts a Typst comment; `=`, `-`, `+` at line start form headings and
  lists (inference from Typst syntax, verified against the escape set —
  the crates have no tests covering these characters).

### String-literal context (inline code, URLs, function arguments)

Inline code is emitted as `#raw("...")`, a Typst string literal. There the
markup escape is wrong and a different one applies: `\` becomes `\\` and
`"` becomes `\"`. Nothing else. Getting this wrong was a real bug
(mdbook-typst issue 3: a lone backslash in inline code broke out of the
tag); the fix and its regression tests exist in both crates.

The same string-literal context applies to link URLs (`#link("url")`),
image paths (`image("src")`), and document metadata — but the crates apply
**no escaping at all** there. A URL or image path containing `"` or `\`
breaks out of the argument list. Image captions and quote attributions are
spliced into content blocks (`caption: [...]`, `attribution: [...]`)
unescaped as well. A fresh implementation should treat every interpolation
point as one of the two contexts and escape accordingly.

### Code blocks: no escaping, tracked by depth

Inside code blocks, text passes through verbatim. Both the converter and
the serializer independently track "am I inside a code block" with a depth
counter pushed/popped on code-block start/end events, because escaping
decisions are made per text event, far from the enclosing tag.

## Element mapping

| Markdown | Typst output |
|---|---|
| Paragraph | `#par()[` ... `]\n` |
| Heading level n | `=` × n, space, text, optional ` <label>`, `\n` |
| Emphasis / strong / strikethrough | `#emph[...]` / `#strong[...]` / `#strike[...]` |
| Inline code | `#raw("...")` with string-literal escaping |
| Fenced/indented code block | fence of 6+ backticks, language directly after the opening fence, verbatim content, matching fence |
| Bullet list item | `- ` at line start, `\n` after item |
| Ordered list item | `+ ` at line start, `\n` after item |
| Block quote | `#quote(block: true, quotes: auto,)[` ... `]\n` |
| Inline/reference/shortcut/collapsed/wiki link | `#link("url")[label content]` |
| Autolink / email | `#link("url")[url text]`, email prefixed with `mailto:` |
| Internal label link | `#link(<label>)[...]` (unquoted label reference) |
| Image, no alt | `#image("src")` or `#align(center)[#image(...)]` |
| Image with caption | `#figure(image("src", width: ..., height: ...), caption: [text])` |
| Table | `#table(columns: N, align: (left, center, ...), ...)` |
| Table header row | `table.header([cell], [cell], ),` |
| Table body cells | `[cell], ` per cell, newline per row |
| Soft break | a single space character |
| Hard break | `#linebreak()\n` |
| Standalone anchor/label | `#[] <label>` (label attached to empty content) |
| HTML `<img>` | parsed with an HTML tokenizer, re-emitted as image events |
| HTML `<sup>`/`</sup>` | raw `#super[` / `]` |

Details worth keeping:

- **Table columns and alignment** come from the Markdown header row's
  alignment spec; column count is its length. Alignment `none` maps to
  Typst `auto`. Header cells go inside `table.header(...)`; body cells are
  appended as bare content blocks.
- **Image alt vs. title**: the converter uses the Markdown *title*
  (`![alt](src "title")`) as the Typst caption. The actual alt text
  arrives as text events between image start/end and is not consumed by
  the image conversion, so it can leak into the output as ordinary text
  (observed in code; only the title path is tested).
- **HTML `<img>` dimensions**: `width`/`height` attributes and CSS
  `style="width: ...; height: ..."` are both honored; bare numeric values
  get a `pt` suffix. A `class` containing `center` triggers the
  `#align(center)` / `placement: none` figure variants.
- **Email autolinks** need the `mailto:` prefix added by the converter;
  pulldown-cmark does not include it.

### Constructs not supported

The crates silently drop or ignore: task-list markers, footnotes,
horizontal rules (thematic breaks produce no output at all), math
(mdBook-style `\[...\]` MathJax text is detected and *stripped*, with a
TODO to translate it to Typst math), definition lists, and raw HTML other
than the `<img>`, `<sup>`, and `<a id=...>` special cases. Ordered-list
start numbers are captured in the event model but never rendered — `+`
markers make Typst renumber from 1, so `6. foo` loses its start value.
Tight vs. loose list layout is likewise modeled but never emitted (an open
TODO). A fresh implementation should decide explicitly per construct:
translate, drop with a warning, or error.

## Markup vs. function calls

The serializer uses Typst *markup* shorthand only where the construct is
line-anchored and self-delimiting: headings (`=`), list items (`-`/`+`),
and code fences (backticks). Everything span-level or parameterized is a
*function call*: `#par()`, `#emph[]`, `#strong[]`, `#strike[]`, `#link()`,
`#quote()`, `#table()`, `#image()`/`#figure()`, `#linebreak()`,
`#pagebreak()`. The project history records an early deliberate switch
"from using typst markup to markup functions" for span styling. The
practical property (our inference, consistent with the escape set): `*` and
`_` must be escaped in text anyway, so emitting them as delimiters would
collide with their escaped occurrences, and function-call brackets are
immune to adjacent characters and word-boundary rules.

Paragraphs as explicit `#par()[...]` rather than blank-line-separated text
make output insensitive to accidental blank lines, but create a new
problem: block content inside a paragraph (see pitfalls).

## Tricky interactions and pitfalls

1. **Inline code containing backticks/backslashes.** Emitting inline code
   as `#raw("...")` instead of backtick markup sidesteps backtick counting
   entirely; only `\` and `"` need escaping. This is the crates' most
   battle-tested decision (regression tests cite the original bug).

2. **Code blocks containing fences.** Code fences are emitted with six
   backticks minimum so content with triple backticks (e.g. Markdown
   examples or Rust doc comments) cannot close the fence, and the count
   grows with nesting depth. Six is still a guess — a code block containing
   six or more backticks would break; counting the longest backtick run in
   the content is the robust fix (our inference).

3. **Block elements inside `#par()[...]`.** Markdown wraps standalone
   images in paragraphs; splicing a `#image()`/`#figure()` into a Typst
   paragraph provokes "block element inside paragraph" warnings. The
   crates run a dedicated filter that buffers each paragraph, checks
   whether it contains only an image (whitespace-only text and soft breaks
   allowed), and drops the wrapper if so.

4. **Balanced start/end events for self-closing constructs.** An HTML
   `<img>` inside a table cell once emitted an image start with no end;
   the serializer's tag-matching stack then paired the cell's end tag with
   the image and panicked. Any construct injected mid-stream must emit
   balanced events.

5. **pulldown-cmark 0.13 end events carry less data than start events.**
   Link ends lack the URL, table ends lack alignment, heading ends lack
   the id, code-block ends lack the fence, list ends lack the start
   number. Anything needed at close time must be saved from the start
   event; the serializer's stack comparison had to be relaxed to
   compare tag *types* only.

6. **Soft vs. hard breaks.** Soft breaks become a plain space (a raw
   newline could interact with Typst's line-anchored markup); hard breaks
   become `#linebreak()`. Mapping hard breaks to anything else was a
   reported bug (mdbook-typst issue 11).

7. **Block quotes need a trailing newline** after the closing bracket or
   following content runs into the quote (fixed after mdbook-typst
   issue 15). The serializer generally terminates every block construct
   (heading, item, paragraph, quote, table, image) with `\n`; inline
   constructs emit none.

8. **Adjacent lists merge.** List end emits nothing; separation between
   consecutive lists is only the items' trailing newlines, and contiguous
   `- ` lines are one list to Typst (our inference from the serializer's
   output; untested in the crates). Nested lists are likewise emitted
   without indentation, so Typst cannot see the nesting.

9. **Labels must attach to content.** A bare `<label>` is invalid; the
   crates attach standalone anchors to empty content as `#[] <label>`.
   Heading labels go after the heading text: `= Title <label>`.

10. **Heading ids.** Ids are slugified mdBook-style: lowercase, keep
    alphanumerics and underscores, spaces and hyphens become hyphens,
    everything else is dropped, hyphens are not collapsed, leading and
    trailing hyphens trimmed. Inline code inside a heading contributes its
    text to the id. Ids must be deduplicated across files (the crates
    prefix them with a file-derived label) and internal links must apply
    the *same* algorithm to anchor fragments so `#link(<...>)` targets
    resolve.

11. **Heading depth.** Markdown caps at 6; Typst `=` repetition is
    unbounded and the serializer would happily emit any level. When
    shifting heading levels (the mdBook layer offsets by chapter depth),
    the crates clamp at 6.

12. **Emphasis nesting** is unproblematic with function calls:
    `#emph[#strong[...]]` nests cleanly, covered by tests.

## Ideas for our implementation

- Escape by context, not globally: one function for markup text, one for
  string literals (`\` and `"`), and none inside raw/code. Model every
  interpolation point (URL, path, caption, label, metadata) as one of the
  two.
- Extend the markup escape set beyond the crates': add `\`, `[`, `]` at
  minimum; consider line-start `=`, `-`, `+` and `/` (comment) handling.
  Drop the space-inserting underscore hack — escape `_` cleanly.
- Prefer `#raw("...")` for inline code and function calls for span
  styling; reserve markup shorthand for headings, list items, and fences.
- Size code fences from the content: longest backtick run + 1 (minimum
  the language-tag-compatible 3), rather than a fixed 6.
- Track code-block depth once, in the layer that decides escaping.
- Keep start-event data (URL, alignment, fence, list start) yourself;
  never rely on end events for it.
- Emit balanced start/end for anything synthesized mid-stream, and end
  every block construct with a newline.
- Unwrap paragraphs whose only content is a block element (images) before
  serializing.
- Decide explicitly what happens to task lists, footnotes, rules, math,
  and raw HTML — the crates' silent drops are their least discoverable
  behavior. Thematic breaks map naturally to `#line()`, which the Typst
  event model already provides but nothing uses.
- If ordered-list start numbers matter, `+` markers cannot express them;
  use `#enum(start: n, ...)`.

## Credits and license

This document distills knowledge gained from reading the source of two
crates by Christian Legnitto (GitHub: LegNeato):

- `pulldown_typst` v0.6.0 — the Typst event model and markup serializer.
- `pullup` v0.4.1 — the Markdown-to-Typst event converters.

Both live in the repository <https://github.com/LegNeato/pullup>; the
analysis used commit `e9ffe5b1435fabdd0fe4f5e3cd5b9097fac6eb9f`, which is
the exact commit the `pulldown_typst` 0.6.0 crates.io release was packaged
from (per the tarball's VCS metadata), cross-checked against the published
tarball. Issue references above point to the consuming project
<https://github.com/LegNeato/mdbook-typst>, whose bug reports drove
several of the fixes.

Both crates declare the license expression `MIT OR Apache-2.0` in their
`Cargo.toml` (author field: `Christian Legnitto <christian@legnitto.com>`).
The repository and the published tarball contain no license file and no
copyright notice line at the analyzed commit, so no copyright line can be
quoted here.

No code or architecture from these crates is reused in ntropy; this
document records only problem-domain knowledge learned from reading them.
