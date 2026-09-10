# Design notes

The design direction as it currently stands, derived from
[work-order.md](work-order.md), [codebase-analysis.md](codebase-analysis.md),
[external-facts.md](external-facts.md), and the decisions in
[decisions.md](decisions.md). Every statement below is a decision from
that log (the Q numbers name them) unless marked open. This document is
rewritten, not appended to, whenever a decision changes it.

## Status

Rounds 1 to 17 of the questions are decided. No question is pending.
Two values stay unfixed until implementation, listed at the end.

## Architecture

- The export is a classic static-site generator: Rust pre-renders every
  page of the site at export time (Q1).
- Browser-side code exists for search, the heading outline, the theme
  toggle, and highlighting. It is written in TypeScript with Preact,
  built with Vite, tested with Vitest (Q2, Q39, Q40). The built output
  is committed under `src/site/dist/` and embedded into the binary; the
  Cargo build never runs the JavaScript toolchain (Q2, Q41). A CI job
  rebuilds the frontend and fails on a difference from the committed
  output; a `just` recipe runs the same check locally (Q46).
- The site works when opened from the filesystem without an HTTP
  server, search included (Q3). Therefore data the browser-side code
  needs is embedded as script files rather than fetched, every page is
  a real `.html` file, links are relative, and there is no client-side
  routing.
- Frontend sources (package manifest, TypeScript, CSS, tests) live in
  `site/` at the repository root (Q41). They carry the MPL-2.0 header
  in their comment syntax; the generated files under `src/site/dist/`
  carry none; a test enforces both (Q44).

## Command surface

- `ntropy site -o <dir> [query]` exports the vault (Q26). The output
  directory is required; a non-empty one is refused unless a force
  flag is given, in which case it is emptied before writing (Q18).
  The optional DSL query restricts the exported notes; a link to a
  note outside the set renders as unresolved (Q8).
- `ntropy site theme init <name>` writes the built-in site theme's
  files to `.ntropy/themes/site/<name>/` and refuses to overwrite an
  existing directory (Q43).
- `render` gains an `html` format registered in the existing
  format/engine registry (Q4). It writes one self-contained HTML file:
  the theme's stylesheet inlined, the note content and frontmatter
  block, note links targeting `<slug>.html` beside the artifact, the
  HTML counterpart of the `<slug>.pdf` convention. No sidebar, no
  search (Q33). Its theme is selected as `ntropy site` selects it:
  `[site] theme`, `--theme` overriding, resolved in `themes/site/`;
  the `pdf` and `typst` formats keep `[render] theme` and
  `themes/typst/` (Q45).
- The export exports once and exits; no watch mode, no server (Q21).
- An encrypted vault exports like a plaintext one; a warning is
  printed when the output directory lies inside the vault (Q15).
- Scan warnings and export warnings (missing assets, unknown fence
  languages, links outside the exported set) go to stderr. Under
  `--strict` both kinds fail the exit code; the site is written either
  way (Q49).
- `ntropy site -p` prints one line, the path of `index.html`, so
  `open "$(ntropy site -o dir -p)"` works. Without it a completion
  report names the directory, the page count, and the warnings (Q50).

## Configuration

`.ntropy/config.toml` gains a `[site]` table (Q32, Q42):

- `theme`: the site theme name, resolved in `.ntropy/themes/site/`.
- `index`: the ULID of the note that becomes the front page.
- `title`: optional, defaults to the vault directory name.
- `lang`: optional, defaults to `en`, used for the `html` `lang`
  attribute.

## Themes

- The themes folder is split by type: `.ntropy/themes/typst/<name>.typ`
  for Typst themes and `.ntropy/themes/site/<name>/` for site themes
  (Q10, Q31). Existing Typst themes must be moved; `render` reports the
  old location with a hint; the changelog documents the change (Q10).
- In the first implementation a site theme consists of stylesheets and
  static assets over an HTML structure fixed by ntropy (Q9). Template
  override by a theme is a possible later iteration, not decided.
- The binary embeds exactly one default site theme (Q11).
- The light/dark toggle follows the system preference by default; a
  manual choice is remembered in the browser (Q25a). Hiding frontmatter
  fields is a theme's job via CSS (Q22).

## Site structure and navigation

- **Front page.** `[site] index` names the note; without it the export
  generates an overview (Q5) showing the site title, the note count,
  the newest N notes, the top-level tags with counts, and the
  configured views, each linking into its index page (Q48). N is not
  fixed.
- **Sidebar.** Built from the configured views and the tag hierarchy:
  each becomes a collapsible section whose groups nest as the
  filesystem views do. No hand-curated order (Q6).
- **Sort order.** Notes inside a group, on a tag page, or on a group
  page are sorted newest first (ULID descending). A frontmatter order
  override is a topic for a later iteration (Q27).
- **Generated pages.** The front page, a tag tree index, one page per
  tag, one index per configured view, one page per group nested as the
  filesystem view (Q25b). No chronological all-notes page, no
  recently-modified page.
- **Tag pages.** A tag page lists notes carrying the tag or any
  descendant tag, the `tag:` query's sub-path rule, and lists its
  child tags (Q28).
- **Note page.** Title, tags as chips, every other frontmatter field
  as key/value beneath with nested values (Q22); an outline built from
  the note's headings with the current section highlighted while
  scrolling; previous/next links; breadcrumbs; the light/dark toggle
  (Q25a). A note's first view and group in sidebar order define its
  previous/next links and its breadcrumb, statically (Q29).
- **Backlinks.** None in the first implementation (Q17).

## Pages, URLs, and files

- A note's page is `notes/<slug>.html` (Q13). Only colliding notes are
  named `<slug>-<tail>.html`, the tail being the shortest ULID suffix
  (minimum three characters) that separates them, the rule the view
  leaves already use; a collider's URL can change when a further
  collider appears (Q23).
- The export copies only the files inside the vault that exported
  notes reference through images or links, plus the theme's files. A
  reference outside the vault is a warning (Q14).
- Raw HTML in a note body passes through verbatim (Q16).
- Generated pages sit one directory per node (Q47a): `index.html` at
  the root; `tags/index.html` and `tags/<a>/<b>/index.html` per tag;
  `views/<name>/index.html` and `views/<name>/<group>/<sub>/index.html`
  per group. The `views/` and `tags/` prefixes keep a view named
  `notes` or `tags` from colliding with the note pages.
- Heading ids follow GitHub's rule (Q47b). Lowercase, punctuation
  dropped except hyphens and underscores, spaces become hyphens,
  Unicode letters stay, duplicates get `-1`, `-2`. Anchor links written
  GitHub-style in notes resolve unchanged.

## Markdown to HTML conversion

One structural Markdown walk over pulldown-cmark behind an output
trait, with the existing Typst emitter and a new HTML emitter as its
two implementations (Q34). Three stages:

1. Extract the walk behind the trait as a pure refactor; the
   kitchen-sink snapshot and the escaping corpus stay unchanged.
2. Add the HTML emitter with its own writer (text, attribute, and raw
   contexts) and its own kitchen-sink snapshot.
3. Add heading ids as a mechanism of the shared walk.

Behaviors the HTML output carries: note links resolved by byte span
against the link table, bare-URL detection, heading ids, asset paths
rewritten relative to the page with the referenced files recorded for
copying, callouts and task checkboxes and note links marked for CSS,
code blocks carrying their language, raw HTML passed through.

## Rich content

- Code blocks are highlighted in the browser by Shiki shipped inside
  the exported site with the JavaScript regex engine; code is plain
  text until scripts run (Q24).
- A curated grammar set is embedded in the binary, built by Vite as
  one classic script file per grammar: Shiki's web bundle languages
  plus rust, go, python, ruby, java, kotlin, swift, c, cpp, csharp,
  shellscript, powershell, toml, ini, dockerfile, makefile, sql, diff,
  lua, typst, latex (Q37). Each page includes only the grammar scripts
  for the fence languages it contains; a language outside the set
  renders as plain code with an export warning (Q30). Shiki itself
  ships ESM-only with no classic-script build; the per-grammar files
  and their registration are our own construction.
- Math stays off in the parser and renders literally; a `mermaid`
  fence is an ordinary code block. Parity with the Typst engine
  (Q24b).

## HTML page assembly

Pages are assembled by minijinja from templates embedded in the
binary (Q35, Q36). Themes supply no templates in the first
implementation (Q9), so the templates are ntropy's own.

## Search

- The query DSL is reimplemented in TypeScript and evaluated in the
  browser over embedded note data (id, title, tags, frontmatter,
  body). `tag:`, `field:`, `and`/`or`/`not`, and parentheses mirror
  the Rust semantics (Q7).
- `text:` maps to JavaScript `RegExp`. A pattern using a construct
  Rust `regex` rejects (lookaround, backreferences) is refused with a
  message naming the construct, so a query that works on the site also
  works in the CLI (Q7).
- One hand-maintained JSON file under `tests/fixtures/` holds the
  conformance cases (query, note fixtures as frontmatter plus body,
  expected matching ids or expected error). The Rust tests and the
  Vitest tests both load it (Q7, Q51).
- A fuzzy narrowing layer over titles and tags, the CLI picker's rows,
  is written in TypeScript. No JavaScript search library, no
  WebAssembly (Q7).

## Guideline: libraries and in-house code both on the table

For every component, existing libraries and an in-house implementation
are both laid out and discussed before choosing. Neither direction is
preferred by default (Q20 with the user's refinement).

## Reuse by the planned desktop application

Nothing is built for the desktop app now. When two options cost about
the same, the one that also serves a later desktop app is preferred,
in the converter, the page rendering, and the browser-side scripts
alike. A later refactor for the desktop app is acceptable (Q12).

## Inspiration

VitePress/Starlight and MkDocs Material are inspiration, not a feature
contract; their features were adopted one by one in Q25a and Q25b
(Q19).

## Testing

Not put to the user. The repository's three insta layers (ADR 0021)
and their precedents are recorded in
[codebase-analysis.md](codebase-analysis.md): `tests/views.rs` for a
generated file tree, `tests/cli.rs` contract tests with redaction, the
kitchen-sink fixture pinned as one snapshot. Vitest covers the
TypeScript side (Q40), including the DSL conformance corpus (Q7).

## Open

- The number of newest notes on the generated front page (Q48).
- The exact curated grammar list as a file in the repository (Q37).
- Later iterations, noted but not designed: template override by
  themes (Q9), frontmatter order override (Q27), backlinks (Q17), math
  and diagrams (Q24b).
