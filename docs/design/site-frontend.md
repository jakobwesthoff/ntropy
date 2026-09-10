# Site frontend

The code that runs in the browser inside an exported site
([site-export.md](site-export.md)): search, the heading outline, the
light/dark toggle, syntax highlighting. Decisions are recorded in
[ADR 0051](../adr/0051-browser-side-code-in-typescript-with-committed-build.md)
(toolchain and layout),
[ADR 0052](../adr/0052-client-side-search-as-a-typescript-query-dsl.md)
(search), and
[ADR 0053](../adr/0053-syntax-highlighting-with-shiki-in-the-browser.md)
(highlighting).

## Toolchain and layout

The frontend is TypeScript with Preact, built with Vite, tested with
Vitest, both run through Bun, which is also the package manager; Node is
not used. Its sources (package manifest, TypeScript, CSS, tests) live in
`site/` at the repository root. Vite writes the built output to
`src/site/dist/`, which is committed and embedded into the binary;
`cargo build` never runs Bun. A CI job rebuilds `site/` with Bun and fails
when the result differs from the committed output; a `just` recipe runs
the same check locally.

Every built file is a classic script or a stylesheet. There are no module
scripts and no lazily loaded chunks, because the site works over
`file://`, where browsers block module loading and `fetch()` of local
files. Data the scripts need (the search data, a page's grammar list) is
embedded in script files.

Source files under `site/` carry the MPL-2.0 header in their comment
syntax; the generated files under `src/site/dist/` carry none. A test
enforces both.

## Search

The search mirrors the CLI's two layers: the query DSL filters the note
set, then a fuzzy layer narrows over titles and tags, the rows the CLI's
picker shows.

The DSL ([query-and-search.md](query-and-search.md)) is reimplemented in
TypeScript over the embedded note data: id, title, tags, frontmatter,
body. `tag:`, `field:`, `and`, `or`, `not`, and parentheses mirror the
Rust semantics. `text:` maps to JavaScript `RegExp`; a pattern using a
construct Rust `regex` rejects, lookaround and backreferences among them,
is refused with a message naming the construct, so a query that works on
the site also works in the CLI. Where both engines accept a pattern,
their semantics can still differ: `\b` is Unicode-aware in Rust and
ASCII-based in JavaScript, and the two recognize different line
terminators. Rust's brace-less `\pL` is not JavaScript syntax.

One hand-maintained JSON file under `tests/fixtures/` holds the
conformance cases: a query, note fixtures as frontmatter plus body, and
the expected matching ids or expected error. The Rust tests and the
Vitest tests both load it, so a drift on either side fails a test.

No search library and no WebAssembly are involved.

## Syntax highlighting

Code blocks are highlighted in the browser by Shiki with its JavaScript
regex engine, shipped inside the exported site. Code is plain text until
the scripts run.

Shiki ships ESM-only; Vite builds it into classic scripts, one file per
grammar, and each page registers its grammars with the highlighter. The embedded grammar set is Shiki's
web bundle languages plus rust, go, python, ruby, java, kotlin, swift, c,
cpp, csharp, shellscript, powershell, toml, ini, dockerfile, makefile,
sql, diff, lua, typst, latex. Each page includes only the grammar scripts
for the fence languages it contains. A fence language outside the set
renders as plain code, and the export warns.

## Outline and theme toggle

The outline lists the note's headings by their ids and highlights the
section currently in view while scrolling. The light/dark toggle follows
the system preference by default and remembers a manual choice in the
browser; the theme supplies both palettes.
