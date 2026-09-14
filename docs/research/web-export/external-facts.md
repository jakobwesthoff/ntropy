# Facts about external libraries

Facts gathered from outside the repository that the design rests on.
Each fact names its source and the date it was checked. Sizes measured
by us are marked as such. Nothing here is a decision.

## Shiki (checked 2026-09-11)

- Current version 4.4.3 of `shiki` and all its sub-packages
  (`@shikijs/core`, `@shikijs/langs`, `@shikijs/themes`,
  `@shikijs/engine-javascript`, `@shikijs/engine-oniguruma`,
  `@shikijs/langs-precompiled`). Source: the npm registry.
- `shiki` is published ESM-only (`"type": "module"`); its exports map
  has no CommonJS, UMD, or IIFE entry. The only documented browser use
  is `<script type="module">` importing from a CDN (esm.sh, esm.run).
  No documentation covers a classic `<script src>` build or offline
  self-hosting. Source: the npm registry `exports` field;
  <https://shiki.style/guide/install#cdn-usage>.
- `createHighlighterCore` from `shiki/core` takes explicit `themes`,
  `langs`, and `engine`; each language or theme entry may be an
  imported module, a dynamic import, a getter, or a plain parsed
  grammar/theme JSON object. `shiki/core` includes no themes,
  languages, or WebAssembly. Languages and themes can be added after
  creation with `loadLanguage` and `loadTheme`; using an unloaded
  language throws. Source: <https://shiki.style/guide/bundles#fine-grained-bundle>,
  <https://shiki.style/guide/install>.
- The JavaScript regex engine needs no WebAssembly; it converts
  Oniguruma patterns to native `RegExp` with `oniguruma-to-es`. It is
  strict by default (throws on a pattern it cannot convert;
  `forgiving: true` suppresses that). As of Shiki 3.9.1 all built-in
  languages are supported by it; best results need the `RegExp` `v`
  flag (ES2024), with a fallback to `u`. The docs recommend it for
  browser use and bundle-size control, and Oniguruma for Node or build
  time. Source: <https://shiki.style/guide/regex-engines>.
- `@shikijs/langs-precompiled` exists (8.5 MB unpacked) but the docs
  warn it is not yet supported due to a known issue
  (<https://github.com/shikijs/shiki/issues/918>). Source: same page;
  the npm registry.
- Sizes stated by the docs: `shiki/bundle/full` 6.4 MB minified,
  1.2 MB gzip; `shiki/bundle/web` 3.8 MB minified, 695 KB gzip, both
  including async chunks. Source: <https://shiki.style/guide/bundles>.
- Sizes measured by us from the published `dist` files (raw / gzip):
  `@shikijs/core` `index.mjs` 50 KB / 12 KB; `@shikijs/langs`
  `rust.mjs` 17 KB / 2.7 KB; `typescript.mjs` 191 KB / 16 KB;
  `javascript.mjs` 185 KB / 16 KB. Unpacked npm package sizes:
  `@shikijs/langs` 8.65 MB, `@shikijs/themes` 1.48 MB,
  `@shikijs/engine-javascript` 10.7 KB, `@shikijs/engine-oniguruma`
  644 KB. Source: unpkg downloads and the npm registry.
- The `web` bundle contains 52 language ids, among them `c`, `cpp`,
  `css`, `html`, `java`, `javascript`, `json`, `markdown`, `php`,
  `python`, `shellscript`, `sql`, `typescript`, `xml`, `yaml`, and web
  frameworks. Rust is not in it. Source:
  `packages/shiki/src/langs-bundle-web.ts` on the `main` branch of
  <https://github.com/shikijs/shiki>.
- Dual themes: passing `themes: { light, dark }` to `codeToHtml`
  emits inline colors for the default theme plus CSS custom properties
  (`--shiki-dark`) for the other; switching is done by a media query
  or a class selector, or with `defaultColor: 'light-dark()'`.
  Source: <https://shiki.style/guide/dual-themes>.

What follows for this project (our reading, given the Q2 and Q3
decisions): the browser-side code is built with our own bundler, so
Shiki's ESM packages can be bundled into classic scripts by that
build; per-grammar files and a global registry would be our own
construction, not a Shiki feature.

## pulldown-cmark's HTML renderer (checked 2026-09-11)

From `pulldown-cmark/src/html.rs` at tag `v0.13.0` in
<https://github.com/pulldown-cmark/pulldown-cmark>; the crate's `html`
feature enables it and pulls in `pulldown-cmark-escape` (registry
index: default features are `getopts` and `html`; ntropy builds with
`default-features = false`).

- A block quote with a GFM kind renders as
  `<blockquote class="markdown-alert-<kind>">`; no title element is
  emitted.
- A heading renders `<h<n>>` with optional `id`, `class`, and
  arbitrary attributes taken from the heading event's fields.
- A link renders `<a href="...">` with an optional `title`; nothing
  else, no class.
- A fenced code block renders `<pre><code class="language-<lang>">`.
- A task marker renders `<input disabled="" type="checkbox" checked=""/>`
  or the unchecked form.
- A footnote reference renders
  `<sup class="footnote-reference"><a href="#name">n</a></sup>`; a
  definition `<div class="footnote-definition" id="name"><sup
  class="footnote-definition-label">n</sup>…</div>`.
- `Event::Html` and `Event::InlineHtml` are written raw, unescaped.

## Template engines (checked 2026-09-11)

From the crates.io registry index and API, and docs.rs.

- **minijinja 2.24.0** (Apache-2.0; 3.0.0-alpha.0 exists). Required
  dependencies: `serde`, `memo-map`; eight optional ones. Stated goals:
  "Well documented, compact API", "Minimal dependencies, reasonable
  compile times and decent runtime performance", "Stay close as
  possible to Jinja2". Templates are added from strings with
  `add_template`; `path_loader` loads from a directory. Inheritance
  (`extends`, `block`), `include`, and `import` are provided by the
  `multi_template` feature. Context is any `serde`-serializable value;
  a `context!` macro builds one. A `default_auto_escape_callback`
  decides escaping by template file extension; the specific
  extensions were not confirmed from the docs excerpt. Source:
  <https://docs.rs/minijinja/2.24.0/minijinja/>,
  <https://crates.io/api/v1/crates/minijinja/2.24.0/dependencies>.
- **tera 2.3.0** (MIT; the 1.x line is at 1.20.1). Required
  dependency: `serde`; seven optional ones (`ahash`, `globset`,
  `indexmap`, `itoa`, `pulldown-cmark-escape`,
  `unicode-segmentation`, `walkdir`). Its feature set was not
  examined. Source:
  <https://crates.io/api/v1/crates/tera/2.3.0/dependencies>.

## Browser search libraries (checked 2026-09-11)

Versions and licenses from the npm registry; sizes from bundlephobia
(minified / minified+gzip); features from each project's
documentation.

| Library | Version | License | Size | Prebuilt index loadable from an embedded object | Field filter | Fuzzy / prefix | Boolean |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| MiniSearch | 7.2.0 | MIT | 18 KB / 6 KB | yes: `toJSON` / `loadJSON` | `fields`, `filter` predicate | both | `combineWith` AND / OR / AND_NOT |
| FlexSearch | 0.8.212 | Apache-2.0 | 50 KB / 17 KB | partial: chunked `export`/`import`; `serialize` experimental, not for `Document` | `Document` index with tags | both | `Resolver` and / or / xor / not |
| Fuse.js | 7.5.0 | Apache-2.0 | 26 KB / 9 KB | yes: `createIndex`, `parseIndex` | weighted `keys` | fuzzy (Bitap); prefix via extended search | extended search tokens and `$and`/`$or` |
| Lunr.js | 2.3.9 | MIT | 28 KB / 8 KB | yes: `JSON.stringify(idx)`, `Index.load` | `field:term` | `term~1`, `term*` | `+` / `-` presence modifiers only |
| Orama | 3.1.18 | Apache-2.0 | 75 KB / 24 KB | yes via `@orama/plugin-data-persistence` JSON mode; an open issue reports Node-only code pulled into bundles | `where` clauses, facets | `tolerance`; prefix default | `threshold` only, no NOT |
| Pagefind | 1.5.2 | MIT | runtime generated per site | no: loads index chunks by HTTP `fetch`; maintainer states it does not work over `file://` (issue 202) | `data-pagefind-filter` | ranking only | filters only |
| Elasticlunr | 0.9.5 (2016) | MIT | 17 KB / 5 KB | yes | `fields` | prefix expansion only | AND / OR, no NOT |

Sources: <https://lucaong.github.io/minisearch/>,
<https://github.com/nextapps-de/flexsearch>, <https://www.fusejs.io/>,
<https://lunrjs.com/guides/>, <https://docs.orama.com/>,
<https://pagefind.app/docs/>, <https://github.com/Pagefind/pagefind/issues/202>,
<https://github.com/weixsong/elasticlunr.js>, <https://bundlephobia.com/>.

None of these implements the ntropy query language; each is a
tokenizing full-text index with its own query syntax. None does regex
matching over bodies.

## The WebAssembly route (checked 2026-09-11)

- `regex` 1.13.1 depends only on `aho-corasick`, `memchr`,
  `regex-automata`, `regex-syntax`. Its docs say nothing about
  WebAssembly. A third-party demo built it for `wasm32-unknown-unknown`
  with default features (regex 1.6.0, 2022). Source:
  <https://github.com/rust-lang/regex>,
  <https://github.com/amatveiakin/regex-wasm-demo>.
- Measured WebAssembly size contribution of `regex` 1.6.0 in that
  demo (uncompressed): 214 KiB with limited Unicode and no `perf`
  features, up to 623 KiB with default features. Source:
  <https://github.com/rust-lang/regex/issues/913>.
- `regex-lite` 0.1.9 trades size for functionality: ASCII-only case
  folding and Perl classes, no `\p{...}`, `&str` haystacks only,
  slower. Source: <https://docs.rs/regex-lite/0.1.9/>.
- `wasm-bindgen` 0.2.128 with a CLI that must match the crate version
  exactly; `wasm-pack` 0.15.0 (2024). Source: crates.io,
  <https://github.com/rustwasm/wasm-pack/releases>.
- `--target no-modules` emits a classic-script global instead of an
  ES module, with documented limitations (no JS snippets, no split
  modules). Issues 3355 and 4103 track its deprecation in favor of
  `--target web`, which is an ES module. Source:
  <https://rustwasm.github.io/docs/wasm-bindgen/reference/deployment.html>,
  <https://github.com/wasm-bindgen/wasm-bindgen/issues/3355>.
- The default init fetches the `.wasm` file, which fails over `file://`
  in Chrome (issue 261). `WebAssembly.instantiate` accepts bytes, so
  embedding the module as base64 in a script avoids the fetch; shown
  working in a blog post, not in the wasm-bindgen guide. Source:
  <https://github.com/rustwasm/wasm-bindgen/issues/261>,
  <https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/instantiate_static>,
  <https://bartbroere.eu/2025/03/06/inlining-wasm-in-html-not-terrible/>.

## Rust `regex` versus JavaScript `RegExp` (checked 2026-09-11)

Relevant to a TypeScript reimplementation of the `text:` predicate.
Sources: <https://docs.rs/regex/latest/regex/#syntax> and MDN.

- Unicode classes: Rust accepts `\pL` and `\p{Greek}`; JavaScript needs
  the `u` or `v` flag, always braces, and `Script=` keys.
- Inline flags `(?i)`: Rust supports them; in JavaScript the RegExp
  modifiers proposal is Stage 4, shipped in Chrome 125, and covers
  only `i`, `m`, `s`.
- Lookaround: JavaScript has it; Rust `regex` rejects it by design.
  A pattern valid in the browser can be invalid in the CLI.
- `\b`: Unicode-aware in Rust by default; ASCII `\w`-based in
  JavaScript.
- Multi-line: both have an `m` flag; JavaScript also treats `\r`,
  U+2028, and U+2029 as line terminators, Rust only `\n` unless the
  `R` flag is set.
