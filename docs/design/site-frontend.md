# Site frontend

The code that runs in the browser inside an exported site
([site-export.md](site-export.md)): search, the heading outline, the
color scheme switch, the drawer's keyboard handling, syntax
highlighting. Decisions are recorded in
[ADR 0051](../adr/0051-browser-side-code-in-typescript-with-committed-build.md)
(toolchain and layout),
[ADR 0052](../adr/0052-client-side-search-as-a-typescript-query-dsl.md)
(search), and
[ADR 0053](../adr/0053-syntax-highlighting-with-shiki-in-the-browser.md)
(highlighting).

## Toolchain and layout

The frontend is TypeScript with Preact, built with Vite, tested with
Vitest, linted and formatted with Biome, all run through Bun, which is
also the package manager; Node is not used. Its sources (package
manifest, TypeScript, tests) live in `site/` at the repository root; the
stylesheet is part of the built-in theme under `src/site/theme/`
([site-export.md](site-export.md), "Themes"). Vite writes the built output to
`src/site/dist/`, which is committed and embedded into the binary;
`cargo build` never runs Bun. A CI job rebuilds `site/` with Bun and fails
when the result differs from the committed output; `just site-check` runs
the same check locally (`site-build`, `site-test` for the type check,
lint, and tests, and `bun run coverage` in `site/` for a coverage report).

Every built script is a classic script. There are no module scripts and
no lazily loaded chunks, because the site works over
`file://`, where browsers block module loading and `fetch()` of local
files. Data the scripts need (the search data, a page's grammars) is
embedded in script files.

The build produces two files. `app.js` is the page script, one
self-contained IIFE holding the scheme switch, the drawer handling, the
outline tracking, search, and the highlighter runtime with Shiki's core,
its JavaScript regex engine, and the two GitHub themes. `grammars.zst` holds every
grammar the curated languages need, their embedded languages included
(110 grammars for the 72 curated ones), as one JSON array compressed with
zstd; the curated list itself is `site/grammars.json`. The build is
byte-stable, which is what lets CI compare a rebuild with the committed
output.

Every page of a site loads `assets/app.js` with `defer`; a page whose
code blocks use a language with a grammar also loads
`assets/grammars/<name>.js` for that grammar and each grammar it embeds,
before the page script, since deferred scripts run in document order and
the page script highlights as soon as it runs.
The export writes those scripts, inflating the blob with the pure-Rust
`ruzstd` crate and wrapping each grammar's JSON in a script that appends
it to `window.__ntropyGrammars`. A fence language without a grammar is
an export warning and its block stays plain. The Bun version the build
runs under is pinned in `site/.bun-version`, which CI reads.

Source files under `site/` carry the MPL-2.0 header in their comment
syntax; the generated files under `src/site/dist/` carry none. A test
enforces both.

The binary embeds `src/site/dist/` and the built-in theme
`src/site/theme/` through a build script that walks both directories
into generated tables, so a file added to either ships without being
listed. Each file is zstd-compressed with the `ruzstd` encoder, a build
dependency only, and stored compressed when that is smaller, raw
otherwise (the fonts and the grammar blob are compressed formats
already); the export inflates on demand with the same crate.

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

The panel is a Preact component mounted into the header's `[data-search]`
element, whose attributes name the search data script and the page's
`../` prefix. Nothing loads until the reader opens the panel; the first
opening injects the data script into the document, which works over
`file://` where `fetch()` of a local file does not. The panel has two
boxes. The first takes the query; a syntax error or a refused regex
construct shows under it as the message the parser produced. The second
narrows the query's results fuzzily over the text the CLI's picker shows
for a row, date, title, and tags, with matches ordered by score and
ties keeping the newest-first order. Each result links to the note's page
and to the page of each of its tags. The list shows at most 50 results.
Escape closes the panel.

`/` anywhere on a page opens the panel with the query box focused, and
Escape closes it; the search button shows the key.

No search library and no WebAssembly are involved.

## Syntax highlighting

Code blocks are highlighted in the browser by Shiki with its JavaScript
regex engine, shipped inside the exported site. Code is plain text until
the scripts run. The page script creates the highlighter from the
grammars the page registered, resolves each block's fence language by
grammar name or alias, and replaces the block with Shiki's markup. Both
themes' colors travel as custom properties on every token, and the
stylesheet picks one per color scheme, so the scheme switch changes
highlighted code with the rest of the page.

The curated grammar set is Shiki's web bundle languages plus rust, go,
python, ruby, java, kotlin, swift, c, cpp, csharp, shellscript,
powershell, toml, ini, dockerfile, makefile, sql, diff, lua, typst, latex.
How the grammars are packaged and reach a page is described under
"Toolchain and layout" above.

## Outline, scheme switch, drawer

The outline lists the note's headings by their ids and marks the entry of
the heading the reader is at with `aria-current` while scrolling: the
last heading whose top has passed the reading line, a fifth of the
viewport down.

The scheme switch has three choices, system, light, and dark. Light and
dark set `data-theme` on the root element and are remembered in
`localStorage` under `ntropy-theme`; system removes both, and the
stylesheet follows the OS preference when the attribute is absent. A
one-line inline script in the page head applies the stored choice before
the first paint, so a dark page does not flash light.

The drawer the sidebar becomes on narrow screens needs no script: the
menu link targets `#sidebar` and the stylesheet shows the pane while it
is the target. The script adds what links cannot: Escape closes it, and
following a link inside closes it so the next page does not open with
the drawer over it.
