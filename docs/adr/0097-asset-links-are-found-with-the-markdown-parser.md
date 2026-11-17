# 97. Asset links are found with the Markdown parser

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0028](0028-note-to-note-links-as-standard-markdown-links.md),
whose consequence "ntropy still ships no Markdown parser" no longer holds.

## Context

ADR 0028 finds note links with a regular expression and says ntropy ships no
Markdown parser. Link masking already parses: `src/link/code.rs` runs
`pulldown-cmark` with the renderer's options to find code regions
(`parser_options()` in `src/render/markdown/mod.rs`), so the regular
expression finds candidates and the parser masks code.

The renderers read links that a regular expression of the form
`[text](target)` does not see: reference-style links (`![a][r]` with
`[r]: _assets/<folder>/pic.png`), collapsed and shortcut references, and
angle-bracket targets for names with spaces (`![a](<my pic.png>)`). A check
that reports files nothing links, or links to files that are missing, has to
agree with the renderers on what a link is.

## Decision

The links that reach files are found with a `pulldown-cmark` pass that uses
`parser_options()`, the options the renderers parse with. A link or image is a
local link when its target, as the parser reads it, is not empty, has no URI
scheme, does not start with `#` or `//` and is not a note link. Autolinks are
not local links. Code spans, code blocks and raw HTML hold no link for the
parser and none for this pass. Note links stay with the existing
expression-based extraction of ADR 0028.

Rewriting the name of an asset folder is not tied to the links the parser
finds. It replaces `_assets/<old>/` by `_assets/<new>/` outside code regions,
so a raw `<img>` tag and a reference definition no link uses follow the folder
as well, and the ULID in the folder name makes a false hit implausible.

ADR 0028's sentence "ntropy still ships no Markdown parser" is replaced by
this: link masking and the asset-link pass parse Markdown with the renderers'
options, and note links are still found by expression.
