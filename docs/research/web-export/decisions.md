# Questions, answers, and decisions

The resumption log for the web-export research. Entries are appended in
the order they happened. A decision is recorded only after the user
confirmed it in conversation; a rationale is recorded only when the user
stated one.

Format:

- **Q<n>. <question>** — the question as put to the user, with the
  options offered.
  - *Answer:* what the user chose or said.
  - *Decision:* the resulting decision, in one sentence, if any.

Entries are never rewritten. A "not yet chosen" remark inside an entry
is resolved by a later entry where one exists; the consolidated state
is [design-notes.md](design-notes.md).

## Decided

### Round 1 (2026-09-10)

- **Q1. Site architecture.** Options offered: a data-plus-viewer model
  (Rust exports HTML fragments and JSON manifests, a browser-side
  viewer renders navigation and search, pre-rendered pages as entry
  points; recommended); a classic static generator (every page
  pre-rendered by Rust templates, JavaScript only for search); a pure
  client-side app.
  - *Answer:* classic static generator. The user gave no rationale.
  - *Decision:* every page of the site is pre-rendered by Rust at export
    time; browser-side JavaScript serves search and comparable
    interactive additions, not page construction.
- **Q2. Frontend toolchain.** Options offered: TypeScript with a bundler,
  built output committed to the repository and embedded via
  `include_str!`, CI checking freshness (recommended); vanilla
  JavaScript and CSS with no toolchain; a separate prebuilt frontend
  crate.
  - *Answer:* TypeScript with a bundler, output committed.
  - *Decision:* browser-side code is written in TypeScript and built with
    a bundler; the built assets are committed and embedded into the
    binary; `cargo build` never invokes the JavaScript toolchain.
- **Q3. `file://` viewing.** Options offered: full function from disk
  including search (recommended); only from a static HTTP host; reading
  from disk with search needing HTTP.
  - *Answer:* full function from disk, including search.
  - *Decision:* the exported site works when opened directly from the
    filesystem with no HTTP server, search included. Consequences
    accepted with the option: data embedded as script files rather than
    fetched JSON, relative links only, a real `.html` file per page, no
    client-side routing.
- **Q4. Command surface.** Options offered: a new whole-vault subcommand
  plus an `html` format on `render` (recommended); only a new
  subcommand; growing `render` into a multi-note operation.
  - *Answer:* new whole-vault subcommand plus `render --to html`.
  - *Decision:* a new subcommand exports the vault as a site; the
    single-note `render` gains an `html` format through the existing
    registry; both share one Markdown-to-HTML converter. The
    subcommand's name is not yet chosen.

### Round 2 (2026-09-10)

- **Q5. Entry point.** Options offered: a configured note with a
  generated overview as fallback (recommended); always a generated
  overview; always a designated note.
  - *Answer:* configured note, generated fallback.
  - *Decision:* a vault config entry names the note that becomes the
    front page; without one the export generates an overview page from
    the note set.
- **Q6. Sidebar navigation.** Options offered: each configured view and
  the tag hierarchy as collapsible sidebar sections (recommended); a
  curated table-of-contents note in the style of mdBook's SUMMARY.md;
  both with the TOC note winning; a frontmatter order field.
  - *Answer:* views and tag tree as sidebar.
  - *Decision:* the sidebar is built from the configured views and the
    tag hierarchy, groups nested as in the filesystem views, with no
    hand-curated order. The sort rule for notes within a group is not
    yet chosen.
- **Q8. Note selection.** Options offered: whole vault with an optional
  query filter (recommended); always the whole vault.
  - *Answer:* whole vault, optional query filter.
  - *Decision:* by default every note is exported; an optional DSL query
    restricts the set, and links to excluded notes render as unresolved.

### Round 3 (2026-09-10)

- **Q9. Theme scope.** Options offered: templates plus CSS plus assets
  rendered by a Rust template engine (recommended); CSS and assets only
  over a fixed HTML structure; a layered model with optional template
  override.
  - *Answer (verbatim):* "css and assets only for the first
    implementation maybe templates in a second iteration later on"
  - *Decision:* in the first implementation a site theme consists of
    stylesheets and static assets over an HTML structure fixed by
    ntropy. Template override is a possible later iteration, not
    decided.
- **Q11. Built-in themes.** Options offered: one embedded default that
  can be copied into a vault (recommended); one embedded default, not
  copyable; several embedded looks.
  - *Answer:* one embedded default, copyable into a vault.
  - *Decision:* the binary embeds exactly one default site theme, and a
    command or flag writes its files into the vault as a starting point
    for a custom theme.
- **Q12. Reuse for the desktop app.** Multi-select over: the
  Markdown-to-HTML converter; page rendering (templates and themes);
  browser-side scripts.
  - *Answer:* all three selected, with this clarification (verbatim):
    "Just to be clear, we are currently not implementing this tauri
    side or anything specific to it. So we are not implementing
    anything specifically to be used with or for that. We just want to
    keep this idea in mind so that if we need to decide something now,
    that is more or less the same work, but can help us later we choose
    that route of course. However it is completely fine with me to
    implement our static rendering now as we want and later on refactor
    once we are thinking about the tauri app. So as i have said we just
    keep it in the back of the head while designing and implementing,
    but dont necessarily need to decide strictly with it already
    planned out"
  - *Decision:* nothing is implemented for or specific to the desktop
    app. Where two options cost about the same, the one that also
    serves a later desktop app is preferred, across all three layers.
    Refactoring later for the desktop app is acceptable.

### Round 4 (2026-09-10)

- **Q10. Theme location.** Options offered in round 3:
  `.ntropy/themes/<name>/` as a site theme beside `<name>.typ` with a
  separate config key (recommended); a separate folder such as
  `.ntropy/site-themes/`; one theme name covering both engines.
  - *Answer (verbatim):* "i guess we should introduce a subdirectory as
    a type for the themes folder to seperate here and in the future"
  - *Clarification asked in round 4:* what happens to the existing
    Typst themes at `.ntropy/themes/<name>.typ`. Options: move them to
    `themes/typst/<name>.typ` as a documented breaking change with a
    hint from `render` at the old location (recommended); move with a
    deprecation fallback; keep Typst flat and only give site themes a
    type directory.
  - *Answer:* move them, no fallback.
  - *Decision:* the themes folder is split by type:
    `.ntropy/themes/typst/<name>.typ` for Typst themes and a sibling
    type directory for site themes. Existing Typst themes must be
    moved; `render` reports the old location with a hint; the change is
    documented in the changelog. The name of the site theme type
    directory is not yet chosen.
- **Q13. Per-note URL.** Options offered: `notes/<slug>.html`
  (recommended); `notes/<ulid>.html`; `notes/<ulid>-<slug>.html`;
  `notes/<slug>/index.html`.
  - *Answer:* `notes/<slug>.html`.
  - *Decision:* a note's page is `notes/<slug>.html`. A collision
    between two notes with the same slug needs a disambiguator; the
    rule is not yet chosen (the view leaves use a ULID tail).
- **Q14. Assets.** Options offered: only referenced files plus the
  theme's assets (recommended); everything non-note under `all-notes/`
  and a vault assets directory; referenced files plus a configurable
  include list.
  - *Answer:* only referenced files.
  - *Decision:* the export copies the files inside the vault that an
    exported note's images or links point at, plus the theme's own
    files. A reference to a file outside the vault produces a warning.
- **Q15. Encrypted vaults.** Options offered: export and warn when the
  output lands inside the vault, as `render` does (recommended);
  require an explicit flag; refuse.
  - *Answer:* export, warn when the output lands inside the vault.
  - *Decision:* an encrypted vault exports like a plaintext one; a
    warning is printed when the output directory lies inside the vault.

### Round 5 (2026-09-10)

- **Q16. Raw HTML in note bodies.** Options offered: pass through
  verbatim (recommended); sanitize with an allowlist; drop with a
  warning as the Typst engine does.
  - *Answer:* pass through verbatim.
  - *Decision:* raw HTML in a note body is emitted as written.
- **Q17. Backlinks.** Options offered: a "linked from" section per note
  computed at export (recommended); not in the first implementation.
  - *Answer:* not in the first implementation.
  - *Decision:* the first implementation shows no backlinks.
- **Q18. Output directory.** Options offered: a required `-o` with a
  non-empty target refused unless forced, then emptied (recommended);
  a default `./site` with the same rule; overwrite in place, never
  delete.
  - *Answer:* required `-o`, refuse non-empty unless forced.
  - *Decision:* the output directory is a required argument; a
    non-empty target is refused unless a force flag is given, in which
    case it is emptied before writing.
- **Q19. Reference documentation generators.** Multi-select over
  mdBook, MkDocs Material, Docusaurus, VitePress/Starlight.
  - *Answer:* VitePress/Starlight and MkDocs Material. Added afterwards
    (verbatim): "the mentioned documentation generators are thought of
    more like an idea not a strict set of features. we might want to
    discuss the features of those and see if we even want that or if
    it makes sense"
  - *Decision:* those two are inspiration, not a feature contract. Their
    features are discussed one by one (round 7) and adopted
    individually.

### Round 6 (2026-09-10)

- **Q23. Equal slugs.** Options offered: shortest ULID tail appended to
  colliders only, the rule the view leaves use (recommended); full ULID
  suffix on colliders only; full ULID on every note.
  - *Answer:* shortest ULID tail, colliders only.
  - *Decision:* only colliding notes are named `<slug>-<tail>.html`,
    the tail being the shortest ULID suffix (minimum three characters)
    that separates them; non-colliding notes keep `<slug>.html`.
    Accepted with the option: a collider's URL can change when a
    further collider appears.
- **Q20. Code highlighting, math, diagrams.** Options offered:
  highlighting in Rust at export with math and Mermaid rendered by
  vendored browser libraries (recommended); everything client-side and
  vendored; highlighting only; client-side via CDN.
  - *Answer (verbatim):* "i think shiki would be a good way to do
    highlighting. I think using already existing stuff here is the
    right way to go. Overall we should not fall in the not invented
    here syndrom trap and always at least consider the use of already
    existing well established and battle proven external libraries and
    stuff we can reuse cleanly"
  - *Refinement by the user during round 7 (verbatim):* "existing
    libraries arent always preferred over writing it ourselves, but we
    should discuss and talk about the possibilities without deciding
    one or the other. everythibg must be on the table"
  - *Decision (guideline):* for every component, both existing
    libraries and an in-house implementation are laid out and discussed
    before choosing; neither direction is preferred by default.
  - *State on highlighting:* the user named Shiki. Shiki is a
    JavaScript library; with page rendering in Rust it can run in the
    browser or not at export time. Which is asked in round 7. Math and
    diagrams were not answered and are re-asked.
- **Q21. Preview and watch.** Options offered: one-shot export only
  (recommended); a `--watch` re-export; a built-in preview server.
  - *Answer:* one-shot export only.
  - *Decision:* the first implementation exports once and exits; no
    watch mode, no server.
- **Q22. Frontmatter on the page.** Options offered: all fields like the
  Typst default template (recommended); title, tags, and created date
  only; a configurable list.
  - *Answer:* all fields.
  - *Decision:* a note page shows the title prominently, tags as chips,
    and every other frontmatter field as key/value beneath, nested
    values included. Hiding fields is a theme's job via CSS in the
    first iteration.

### Round 7 (2026-09-10)

- **Q24. Highlighting.** Options offered: syntect in Rust at export
  (recommended); Shiki in the browser, bundled with the site; Shiki at
  export via Node.
  - *Answer:* Shiki in the browser, bundled with the site.
  - *Decision:* code blocks are highlighted in the browser by Shiki
    shipped inside the exported site. Accepted with the option: a
    chosen language set and the JavaScript regex engine are bundled,
    code is plain until scripts run, and the export grows by roughly a
    megabyte. The language set is not yet chosen.
- **Q24b. Math and diagrams.** Multi-select over KaTeX vendored, Mermaid
  vendored, neither.
  - *Answer:* neither in the first implementation.
  - *Decision:* parity with the Typst engine: math stays off in the
    parser and renders as literal text; a `mermaid` fence is an
    ordinary code block.
- **Q25a. Page features.** Multi-select over right-hand page outline,
  previous/next links, breadcrumbs, light/dark toggle.
  - *Answer:* all four.
  - *Decision:* a note page has an outline built from its headings with
    the current section highlighted while scrolling, previous/next
    links, breadcrumbs, and a light/dark toggle (system preference by
    default, manual switch remembered in the browser). The order that
    previous/next follows and the breadcrumb for a note that sits in
    several groups are open (round 8).
- **Q25b. Index pages.** Multi-select over a tag index with a page per
  tag; a page per view and per group; all notes chronological;
  recently modified.
  - *Answer:* tag index with a page per tag; a page per view and per
    group.
  - *Decision:* the generated pages beside the front page and the note
    pages are a tag tree index, one page per tag, one index per
    configured view, and one page per group nested as the filesystem
    view. No chronological all-notes page and no recently-modified
    page.

### Round 8 (2026-09-11)

- **Q29. Previous/next order and breadcrumb.** Options offered: context
  from the page the reader came from, carried by a script
  (recommended); the first membership in sidebar order, always;
  chronological previous/next with breadcrumb per first membership.
  - *Answer:* first membership in sidebar order, always.
  - *Decision:* a note's first view and group in sidebar order define
    its previous/next links and its breadcrumb, statically, regardless
    of how the reader arrived.
- **Q27. Sort rule inside groups.** Options offered: newest first as
  `list` prints (recommended); by title; oldest first.
  - *Answer (verbatim):* "usually newest first, but we might want to
    allow override of order via frontmatter unsure how this could be
    done. first lets go newest first, but lets keep in mind that we
    might want to discuss solve this order problem later on in another
    iteration"
  - *Decision:* newest first (ULID descending) in the first
    implementation. An order override through frontmatter is a topic
    for a later iteration, not designed.
- **Q28. Tag page membership.** Options offered: exact tag plus all
  descendants, matching the `tag:` sub-path rule (recommended); exact
  tag only with children as links.
  - *Answer:* exact tag plus all descendants.
  - *Decision:* a tag page lists notes carrying the tag or any
    descendant tag, and lists its child tags.
- **Q26. Command name.** Options offered: `site` (recommended),
  `export`, `publish`, `web`.
  - *Answer:* `site`.
  - *Decision:* the subcommand is `ntropy site`, taking the output
    directory as a required `-o` argument and an optional query.

### Round 9 (2026-09-11)

- **Q31. Site theme type directory.** Options offered: `site`
  (recommended), `html`, `web`.
  - *Answer:* `site`.
  - *Decision:* site themes live at `.ntropy/themes/site/<name>/`,
    beside `.ntropy/themes/typst/<name>.typ`.
- **Q32. Config keys.** Options offered: a new `[site]` table with
  `theme = "<name>"` and `index = "<ulid>"`, `--theme` overriding per
  invocation like `render` (recommended); a `[render.site]`
  subsection; theme in config with the front page marked by a
  frontmatter flag.
  - *Answer:* `[site] theme`, `[site] index` by ULID.
  - *Decision:* `.ntropy/config.toml` gains a `[site]` table with
    `theme` naming a site theme and `index` naming the front-page note
    by ULID; `ntropy site --theme` overrides the configured theme for
    one invocation.
- **Q33. `render --to html`.** Options offered: a self-contained page
  without site chrome, default or vault theme CSS inlined, note links
  to `<slug>.html` beside it (recommended); the site's note page with
  sidebar; a bare body fragment.
  - *Answer:* self-contained page, no site chrome.
  - *Decision:* `render --to html` writes one HTML file with the
    theme's stylesheet inlined, the note content and frontmatter block,
    and note links targeting `<slug>.html` beside the artifact, the
    HTML counterpart of the `<slug>.pdf` convention. No sidebar, no
    search.

### Round 10 (2026-09-11)

- **Q35. HTML page assembly.** Options offered, laid out without a
  recommendation: a runtime template engine (minijinja or tera) with
  templates embedded in the binary; compile-time templates (askama or
  maud); hand-written assembly in Rust; discuss further first.
  - *Answer:* runtime template engine.
  - *Decision:* pages are assembled by a runtime template engine from
    templates embedded in the binary. Whether minijinja or tera is not
    yet chosen.
- **Q30. Shiki language set.** Round 9 options: Shiki's `web` bundle
  (recommended); a curated list of common languages kept in the repo;
  Shiki's full bundle. Answer (verbatim): "curated list of common
  languages and maybe configurable what languages should be
  available?". Round 10 clarification options: per-page automatic
  inclusion of only the grammars a page's fences use, no config
  (recommended); the same plus vault-supplied grammar files; one
  bundle with the whole set on every page; a config list restricting
  the embedded set.
  - *Answer:* per-page auto-inclusion, no config.
  - *Decision:* a curated grammar set is embedded in the binary, built
    as one script file per grammar. Each exported page includes only
    the grammar scripts for the fence languages it contains. A fence
    language outside the set renders as plain code and produces an
    export warning. The curated list itself is not yet written.

### Round 11 (2026-09-11)

- **Q34. Markdown-to-HTML converter.** Options offered, laid out
  without a recommendation: an in-house emitter over pulldown-cmark
  beside the Typst emitter (1a: copy the skeleton; 1b: extract the
  structural walk behind an output trait shared by both emitters);
  pulldown-cmark's `html` renderer with an event-rewriting pass;
  comrak; discuss further first.
  - *Answer:* discuss further in chat first. After the prose
    discussion the user asked for a recommendation; the assistant
    recommended 1b with three stages (extract the walk as a pure
    refactor with unchanged snapshots; add the HTML emitter with its
    own writer and kitchen-sink snapshot; add heading ids as a shared
    mechanism). The user answered "accepted".
  - *Decision:* one structural Markdown walk behind an output trait,
    with the Typst emitter and a new HTML emitter as its two
    implementations, built in the three stages above. The reasons
    given were the assistant's; the user stated none.

### Round 12 (2026-09-11)

- **Q7. Search semantics.** Round 2 options: DSL parity via
  WebAssembly (recommended then); the DSL reimplemented in TypeScript;
  simple JavaScript search only; simple search plus WebAssembly. The
  user deferred (verbatim): "i am more thinking about a typescript
  reimplementation, but i like the wasm idea as well. We should talk
  about the complexity for this more. or maybe even discuss later on
  what possible existing nice typescript/javascript based libraries
  for this problem exist. Lets discuss this in detail when we are at
  this point". In the dedicated discussion the options were: A, a
  TypeScript reimplementation of the DSL with a shared conformance
  corpus; B, a WebAssembly build of the query module; C, a JavaScript
  search library; combinations of C with A or B. The assistant
  recommended A plus a TypeScript fuzzy narrowing layer over titles
  and tags mirroring the CLI's picker rows, no search library, and
  refusing in the browser the regex constructs Rust rejects
  (lookaround, backreferences) with a message naming the construct.
  The user answered "we go with your recommendation".
  - *Decision:* the query DSL is reimplemented in TypeScript over
    embedded note data; `tag:`, `field:`, and the boolean operators
    mirror the Rust semantics; `text:` maps to `RegExp`, and a pattern
    using a construct Rust `regex` rejects is refused with a message
    naming the construct. A conformance corpus exported from the Rust
    tests is replayed by the TypeScript tests. A fuzzy narrowing layer
    over titles and tags is written in TypeScript. No JavaScript search
    library, no WebAssembly. The reasons given were the assistant's;
    the user stated none.

### Round 13 (2026-09-11)

- **Q37. Curated Shiki grammar list.** Options offered: Shiki's web
  bundle languages plus systems and config languages, about 70
  grammars (recommended); a minimal developer-notebook set; Shiki's
  full set as separate files.
  - *Answer:* web set plus systems and config languages.
  - *Decision:* the embedded grammar set is Shiki's web bundle
    languages plus rust, go, python, ruby, java, kotlin, swift, c,
    cpp, csharp, shellscript, powershell, toml, ini, dockerfile,
    makefile, sql, diff, lua, typst, latex. The exact list is
    maintained in the repository once implementation starts.
- **Q38. Browser-side framework.** Options offered: plain TypeScript
  without a framework (recommended); a small framework (Preact or
  Solid); decide when the search UI is designed.
  - *Answer:* a small framework (Preact or Solid).
  - *Decision:* the browser-side code uses a small UI framework. Which
    of Preact and Solid is not yet chosen.

### Round 14 (2026-09-11)

- **Q36. Template engine.** Options offered: minijinja 2.24.0; tera
  2.3.0. The user asked "what is your recommendation and why". The
  assistant recommended minijinja for: its stated goal of staying
  close to Jinja2 (a syntax documented beyond this project, relevant
  if themes override templates later); two required dependencies;
  inheritance, include, and import; templates loadable from strings
  now and from a directory through `path_loader` for a later
  vault-side override; its author maintains Jinja2. Tera 2.3.0's
  feature set was not examined.
  - *Answer:* minijinja.
  - *Decision:* page templates are rendered with minijinja. The
    reasons were the assistant's; the user stated none.
- **Q39. Browser-side framework.** Options offered: Preact
  (recommended); Solid.
  - *Answer:* Preact.
  - *Decision:* the browser-side code is written in TypeScript with
    Preact.

### Round 15 (2026-09-11)

- **Q40. Bundler and test runner.** Options offered: Vite with Vitest
  (recommended); esbuild with Node's test runner; Bun.
  - *Answer:* Vite and Vitest.
  - *Decision:* the browser-side code is built with Vite and tested
    with Vitest.
- **Q42. Site title and language.** Options offered: optional
  `[site] title` and `[site] lang` with defaults from the vault
  (recommended); title from the README heading; required config.
  - *Answer:* config with defaults.
  - *Decision:* `[site] title` defaults to the vault directory name and
    `[site] lang` to `en`; both optional.
- **Q43. Theme copy-out command.** Options offered:
  `ntropy site theme init <name>` (recommended); a flag on the export
  command; no command.
  - *Answer:* `ntropy site theme init <name>`.
  - *Decision:* `ntropy site theme init <name>` writes the built-in
    theme's files to `.ntropy/themes/site/<name>/` and refuses to
    overwrite an existing directory.

### Round 16 (2026-09-11)

- **Q41. Source and output location.** Round 15 options: `web/` at the
  repository root with output under `src/site/assets/` (recommended);
  everything under `src/site/web/`; a separate repository. Answer
  (verbatim): "why not site/ at repo root and output to
  src/site/assets or src/site/build or src/site/dist?". Round 16
  options for the output name: `src/site/dist/` (recommended),
  `src/site/assets/`, `src/site/build/`.
  - *Answer:* `src/site/dist/`.
  - *Decision:* the frontend sources (package manifest, TypeScript,
    CSS, tests) live in `site/` at the repository root; the built
    output is committed under `src/site/dist/` and embedded by the
    Rust `site` module.
- **Q44. License headers.** Options offered: headers on frontend
  sources, generated output exempt, both enforced by a test as
  `vault::seed` does (recommended); headers everywhere including
  output; sources only, unenforced.
  - *Answer:* headers on sources, generated output exempt, enforced.
  - *Decision:* TypeScript, CSS, and config files under `site/` carry
    the MPL-2.0 header in their comment syntax; files under
    `src/site/dist/` carry none; a test enforces both.
- **Q45. Theme for `render --to html`.** Options offered: the site
  theme selection (`[site] theme`, `--theme` resolved against
  `themes/site/`), with `pdf`/`typst` keeping `[render] theme` and
  `themes/typst/` (recommended); always the built-in default; one
  `[render] theme` key for both.
  - *Answer:* site theme selection.
  - *Decision:* for the `html` format, `render` selects the theme like
    `ntropy site` does: `[site] theme` from config, `--theme`
    overriding, resolved in `themes/site/`. The `--theme` flag's
    meaning follows the format.
- **Q46. CI freshness check.** Options offered: rebuild and diff in
  CI with a matching `just` recipe (recommended); rely on discipline.
  - *Answer:* rebuild and diff in CI.
  - *Decision:* a CI job builds `site/` and fails when the result
    differs from the committed `src/site/dist/`; a `just` recipe runs
    the same check locally.

### Round 17 (2026-09-11)

- **Q47a. Paths of generated pages.** Options offered:
  `views/<name>/<group>/index.html` and `tags/<tag>/index.html`, a
  directory per node (recommended); view names at the site root as in
  the vault, which collides with `notes/` for a view of that name;
  flat `<node>.html` files beside child directories.
  - *Answer:* directory per node under `views/` and `tags/`.
  - *Decision:* `index.html` at the root; `notes/<slug>.html`;
    `tags/index.html` and `tags/<a>/<b>/index.html` per tag;
    `views/<name>/index.html` and
    `views/<name>/<group>/<sub>/index.html` per group.
- **Q47b. Heading ids.** Options offered: GitHub's rule (recommended);
  ntropy's slugify.
  - *Answer:* GitHub's rule.
  - *Decision:* heading ids follow GitHub's rule. Lowercase, drop
    punctuation except hyphens and underscores, spaces become hyphens,
    Unicode letters stay, duplicates get `-1`, `-2`.
- **Q48. Generated front page.** Options offered: an overview with the
  newest notes, top-level tags, and views (recommended); the vault's
  README.md rendered; README followed by the overview.
  - *Answer:* the overview.
  - *Decision:* without `[site] index` the front page shows the site
    title, the note count, the newest N notes, the top-level tags with
    counts, and the configured views, each linking into its index
    page. N is not fixed.
- **Q49. Warnings.** Options offered: follow `render` (recommended);
  warn only; abort before writing under `--strict`.
  - *Answer:* follow `render`.
  - *Decision:* scan warnings and export warnings go to stderr; under
    `--strict` both kinds fail the exit code; the site is written
    either way.
- **Q50. `--print`.** Options offered: `-p` prints the path of
  `index.html` (recommended); no flag.
  - *Answer:* `-p` prints the `index.html` path.
  - *Decision:* `ntropy site -p` prints one line, the path of the
    entry page. Without it a completion report names the directory,
    the page count, and the warnings.
- **Q51. Conformance corpus.** Options offered: hand-maintained JSON
  under `tests/fixtures/` read by both suites (recommended); generated
  by a Rust test; separate corpora.
  - *Answer:* hand-maintained JSON read by both.
  - *Decision:* one JSON file under `tests/fixtures/` holds the cases
    (query, note fixtures as frontmatter plus body, expected matching
    ids or expected error). The Rust tests and the Vitest tests both
    load it.

### Round 18 (2026-09-11)

- **Q52. JavaScript runtime.** Not asked; stated by the user during the
  implementation (verbatim): "we prefer bun over node, always that is
  decided and should be adhered to and be documented".
  - *Decision:* Bun is the JavaScript runtime and package manager for the
    frontend toolchain, running Vite and Vitest (Q40); Node is not used.
    Recorded as an amendment to ADR 0051.

- **Q53. Frontend linting.** Raised by the user during the implementation
  (verbatim): "do we want something like eslint? or are there more modern
  alternatives? Or dont we want that?". Options offered: Biome
  (recommended); oxlint with Biome formatting; ESLint with
  typescript-eslint and Prettier; no linter.
  - *Answer:* Biome.
  - *Decision:* Biome lints and formats the frontend sources in `site/`,
    run through Bun; it is part of the frontend check.

- **Q54. Grammar packaging.** Raised by the measured sizes during the
  implementation: the 72 curated grammars built as self-contained files
  total 19 MB because Shiki's per-language modules carry every grammar a
  language embeds; crates.io refuses crates over 10 MB. Facts put to the
  user: on their own the grammars are 3.3 MB, the closure of 110 grammars
  4.0 MB, gzipped per file 549 KB, gzipped solid 514 KB, zstd solid
  306 KB, xz solid 296 KB, brotli solid 291 KB. The user asked (verbatim)
  "cant we download them as part of the build process, so that we dont
  have them directly in the repository? And why do we need to uncompress
  them? cant we load the gz compressed version in the browser?", then
  "what exactly do we need/use the build script for decompression wise?",
  then "can we maximize compression even more?". Answers given: the
  frontend build already downloads Shiki from npm and commits only its
  output; a browser inflates compressed scripts only behind an HTTP
  server's `Content-Encoding`, never from `file://`; a build script is
  not needed at all with one solid blob whose grammar list and
  embedded-language lists are read from the data. Codec options: brotli
  (recommended), zstd, xz.
  - *Answer:* the solid-blob route ("sounds like a manageble idea") with
    zstd.
  - *Decision:* the frontend build writes the dependency closure of the
    curated grammars as one JSON array compressed with zstd into
    `src/site/dist/grammars.zst`, committed and embedded in the binary.
    At export the blob is inflated with the pure-Rust `ruzstd` crate, the
    grammars a site's fence languages need (with their embedded
    languages) are written as plain scripts under `assets/grammars/`,
    and each page includes only its own. Supersedes the per-grammar
    files of ADR 0053, which is amended.

## Answered but not yet turned into a decision

_Not yet established._

## Open

Assembled from [codebase-analysis.md](codebase-analysis.md) on
2026-09-10. Ordered so that earlier answers narrow later questions.
Questions are asked in rounds of up to four; a question moves to
"Decided" or "Answered" above once the user has responded.

Round 1 (Q1 to Q4) is decided; see above.

### Round 2: navigation and entry point

Q5, Q6, Q8 decided (see above). Q7 was deferred to a dedicated
discussion and decided in round 12.

### Round 3: themes and reuse

Q9, Q11, Q12 decided (see above). Q10 answered; the clarification
(whether existing Typst themes move into a type subdirectory, and the
type names) is part of round 4.

### Round 4: details

Q10 (clarified), Q13, Q14, Q15 decided (see above).

### Round 5

Q16 to Q19 decided (see above).

### Round 6

Q23, Q21, Q22 decided; Q20 partly (see above).

### Round 7

Q24, Q24b, Q25a, Q25b decided (see above).

### Round 8

Q29, Q27, Q28, Q26 decided (see above).

### Round 9

Q31, Q32, Q33 decided; Q30 answered, clarification pending (see above).

### Round 10

Q35 and Q30 decided; Q34 (converter) discussed in prose and decided
in round 11.

### Rounds 11 and 12

Q34 (converter) and Q7 (search) decided after prose discussions.

### Rounds 13 to 16

Q36, Q37, Q38, Q39, Q40, Q41, Q42, Q43, Q44, Q45, Q46 decided (see
above).

### Round 17

Q47a, Q47b, Q48, Q49, Q50, Q51 decided (see above).

### Still open

No question is pending. Two small values stay unfixed until
implementation: the number of newest notes on the generated front
page (Q48) and the exact curated grammar list file (Q37).

- **Later iterations, noted:** template override by themes (Q9),
  frontmatter order override (Q27), backlinks (Q17), math and diagrams
  (Q24b).
