# Custom Typst render engine

## Context

The v1 `pdf` engine (ADR 0038, `src/render/pandoc.rs`) is pandoc reading
GFM with `--pdf-engine=typst`. The only thing pandoc contributes is the
Markdown-to-Typst conversion; typesetting is already typst's job (pandoc
shells out to the `typst` binary). The pandoc indirection has known costs
discussed on 2026-07-10:

- Theming goes through pandoc's template layer (template variables,
  `header-includes`, `-V template=` swapping the `conf` function, or full
  `--template` replacement) instead of a Typst template we own directly.
- Pandoc's stock typst template splices some metadata verbatim into typst
  code; the `keywords` slot is unusable for that reason (documented in
  ADR 0038) and tags currently ride in the `subtitle` slot as a
  workaround.
- Link rendering is constrained to what survives a Markdown round-trip:
  resolved note links are rewritten to emphasized text (`*Title*`) before
  pandoc sees the body, rather than emitting purpose-styled Typst.

## Decided route (user, 2026-07-10)

1. Implement our own Typst render engine that converts notes to Typst
   markup. Before implementing, heavily analyze the `pulldown_typst`
   crate (crates.io, v0.6.0 as of June 2026) for inspiration on the
   conversion, then build our own clean, extendable implementation.
   The analysis result is a generalized document under `docs/research/`
   focused on solved problems and pitfalls of Markdown-to-Typst
   conversion (escaping, event mapping, edge cases), not code structure,
   with credits and license information for the library.
   **Done 2026-07-10**: `docs/research/markdown-to-typst-conversion.md`
   (uncommitted). The mapping logic turned out to live in the sibling
   crate `pullup` v0.4.1 of the same workspace
   (<https://github.com/LegNeato/pullup>, `MIT OR Apache-2.0`); the
   document covers both crates. See below for the findings that feed the
   implementation.
2. Keep the pandoc engine as a backup while the custom engine is built.
   Once the custom engine delivers complete, working, nice PDF
   conversion, the pandoc engine will most likely be removed entirely
   (user, 2026-07-10); until then it stays selectable.
3. With the Typst conversion in place, add `pdf` as another format of the
   new engine by calling the typst typesetter on the converted output.
4. Only after that, start on theming (better default theme, user-provided
   themes).

Planning proceeds bottom-up: base interfaces and implementation discussed
piece by piece with concise example code.

## Decisions since (user, 2026-07-10)

- **Markdown parser: `pulldown-cmark`.** Facts on the table when the
  user decided: both candidates support GitHub callouts/alerts
  (`> [!NOTE]` etc.; pulldown-cmark behind `ENABLE_GFM`, surfaced as a
  kind on the blockquote tag; comrak via its `alerts` extension), both
  are actively maintained (comrak 0.53.0 on 2026-07-02, pulldown-cmark
  0.13.4 on 2026-05-20), and both are widely used with pulldown-cmark
  roughly 19x ahead (116.5M total / 34.3M last-90-days downloads vs.
  comrak's 6.0M / 1.8M, crates.io on 2026-07-10). Recommendation
  arguments raised alongside: pulldown-cmark's pull-based event stream
  fits a streaming emitter (flat loop with explicit state), its GFM
  extension flags match the input dialect ADR 0038 pins, and it is the
  more conservative, more widely vetted dependency; comrak's edge is
  strictest GFM fidelity (cmark-gfm port) and a larger extension set,
  at the cost of an AST walk and a heavier crate.
- **Typst typesetting: external `typst` invocation for now.** The `pdf`
  format of the new engine runs the `typst` binary (fits the existing
  `RenderContext` `run` primitive) rather than embedding the compiler
  crates. Alternatives discussed 2026-07-10: embedded
  `typst`/`typst-pdf`, possibly via the `typst-as-lib` wrapper, would
  remove the external tool dependency but grow the dependency tree,
  compile time, and binary size (embedded fonts alone are several MB).
- **Escaping verification: `typst-syntax` as dev-dependency, curated
  corpus, no proptest.** In-process round-trip tests via the typst-syntax
  parser keep the no-external-tools testing policy; the corpus is
  enumerated systematically from the escape-set tables in the design doc;
  proptest was judged overkill for now. A data-provider test library
  (e.g. rstest) was discussed and deferred: a failure-collecting loop
  plus an insta corpus snapshot covers the current need; revisit if the
  corpus outgrows a single table.
- **Writer/context model: escaping bound to the write method** ("option
  B"): the writer exposes `markup_text` / `string_literal` / `raw`, plus
  a module-private `syntax` channel for emitter-owned markup; the event
  loop keeps a structural stack (needed anyway for start-event data) and
  owns block terminators and newline discipline; the writer stays dumb
  about structure. Recorded in the design doc's "Writer and context
  model" section.
- **Paragraphs: blank-line separation, not `#par()[...]`.** Escaping
  already prevents accidental blank lines (the wrapper's one advantage),
  and the wrapper caused the reference implementation's block-in-
  paragraph problem. Recorded in the design doc's "Element mapping".
- **Element mapping settled 2026-07-10** (design doc carries the table):
  headings, spans, code, lists (explicit ordered-list numbers via `<n>.`
  markers, nested lists indented), block quotes, tables, breaks,
  thematic breaks as `#line(length: 100%)`, footnotes inlined as
  `#footnote[...]`, task lists as `☐`/`☑` lead-ins, note links as
  `#emph[Title]` (pandoc-engine parity; styling hook deferred to
  theming), raw HTML dropped with a render warning. Callouts emit
  `#callout(kind: "...")[...]` backed by a **prelude** in the emitted
  document defining a default `callout`; the prelude is the designated
  theming hook.

## Open questions

- **Math: not supported for now** (user, 2026-07-10). Findings and the
  future build path via the `mitex` LaTeX-to-Typst translator are
  preserved in
  `01kx5n2ww5526gtfmhga2b8xe4-typst-engine-math-support-via-mitex.md`.
  Interim handling decided (user, 2026-07-10): `ENABLE_MATH` stays off;
  `$` renders as literal escaped text, `math` fences as plain code
  blocks, no warnings. Support target for the
  engine generally: what GitHub renders, not the formal GFM spec, and
  explicitly not pandoc parity (the pandoc engine was a stopgap and
  will be deprecated).
- **Extended autolinks: supported** (user, 2026-07-10). Bare
  `https://...` / `www....` URLs are GitHub behavior but pulldown-cmark
  has no option for them (verified empirically with `Options::all()`),
  so the emitter detects them itself over text events and emits
  `#link("...")[...]`; `linkify` crate vs. own regex is decided at
  implementation time.
- **Other GitHub platform features: out of scope** (user, 2026-07-10).
  Mermaid, emoji shortcodes, color chips render as their literal text,
  no warning (a mermaid block is an ordinary code block).
- **Remote images: degrade to a link** (user, 2026-07-10).
  `![alt](https://...)` becomes `#link("url")[alt text]` plus a render
  warning — `typst compile` performs no network access, so remote
  images cannot appear in the artifact; the compile never fails over
  one.
- **Document skeleton settled 2026-07-10** (design doc, "Document
  skeleton"): inlined prelude defining `note` and `callout`, body
  applied via `#show: note.with(...)`, metadata as typed Typst values
  via recursive YAML-to-Typst translation, **all** frontmatter fields
  passed and displayed by the default template (user: all fields, not
  only title/tags/date). One emitted file serves both formats; the pdf
  pipeline compiles the typst format's exact bytes, which favors the
  stdin asset mechanism over `--root` rewriting.
- **PDF-pipeline asset resolution: stdin mechanism, verified.** Decided
  model is paths-verbatim with resolution as if the document sits next
  to the note (user, 2026-07-10). Verified against typst 0.15.0
  (2026-07-10): `typst compile -` treats the stdin document as living
  at the project root, root defaults to the working directory, root is
  also the file-access sandbox, and the stdin document always sits at
  the root. Mechanism: cwd = note's directory, document on stdin —
  vault untouched, pdf compiles the typst format's identical bytes.
  Accepted limitation: `../` assets above the note's directory fail
  with typst's explicit sandbox error; contained future extension is an
  opt-in `--root`-plus-rewriting variant.

## Playground

`playground/typst/` (standalone crate, not part of the ntropy package)
holds the writer, the escape rules, the fence-sizing helper, and the
round-trip verification developed during the design discussion, heavily
documented and intended to be lifted into `src/render/` at implementation
time. Its tests run the full escaping corpus (167 cases) through the
`typst-syntax` 0.15 parser in-process; all pass, so the 20-character
escape set is machine-verified.

## Research findings feeding the implementation

Extracted from the `pullup`/`pulldown_typst` analysis (full detail in
`docs/research/markdown-to-typst-conversion.md`):

- Escaping is the core problem and has two distinct contexts: Typst
  markup text versus string literals in function calls. Conflating them
  was a shipped bug downstream of the reference crate.
- The reference crate's markup escape set is incomplete (`\`, `[`, `]`
  pass through; URLs, image paths, and captions get no escaping), its
  underscore escape inserts a spurious space (do not copy), block
  elements inside `#par()[...]` need unwrapping, and unbalanced
  start/end events once caused a panic.
- It silently drops ordered-list start numbers, tight/loose list
  distinction, task lists, footnotes, horizontal rules, and math — less
  coverage than our GFM surface needs, confirming the decision to build
  our own emitter rather than depend on it.

- **Emitted Typst is an intermediate representation** (user,
  2026-07-10): readable/idiomatic Typst output is a non-goal; the
  foolproof unconditional escaping and function-call forms stay. A
  discussed middle ground (condition-aware minimal escaping, markup
  spans and backtick inline code behind safety predicates, verified by
  the same round-trip harness) was considered and rejected in favor of
  intermediate-only output. Known consequence to revisit with theming:
  escaped `'`/`"` render as straight quotes in the PDF and cannot be
  re-smartened by a theme; typographic quotes would need a narrow
  emitter change (stop escaping quotes).

- **Engine surface settled 2026-07-10** (design doc, "Engine surface"):
  registers as `typst`, produces formats `typst` and `pdf`; becomes the
  `pdf` default when ready; `Invocation` grows a stdin payload and a
  working-directory field, both recorded in the fake context's
  snapshot transcript.

## Design document

`docs/design/typst-engine.md` (started 2026-07-10) carries the decided
route and the full escaping design (Typst's markup-active surface from
the typst-syntax lexer, the three output contexts, the unconditional
20-character markup escape set, string-literal and raw rules, and the
function-calls-for-span-styling guarantee).

## Fit with the existing architecture

ADR 0038's `Renderer` trait plus format/engine registry holds multiple
engines per format, so the custom engine registers alongside `Pandoc`
and the default per format can change without removing the backup.
