# The typst engine

The engine behind the `render` command's `typst` and `pdf` formats:
ntropy converts the note body to Typst markup with its own emitter and
delegates only typesetting to the `typst` binary. The command surface,
shared preparation, execution model, and testing seam are described in
[rendering.md](rendering.md); the engine decision is recorded in
[ADR 0040](../adr/0040-custom-typst-engine-with-own-markdown-emitter.md).
Pitfall research distilled from a prior converter implementation lives in
[markdown-to-typst-conversion.md](../research/markdown-to-typst-conversion.md).

## Shape

- The emitter is built on the `pulldown-cmark` parser. The supported
  input surface is what GitHub renders, mapped to `pulldown-cmark`
  options flags (tables, strikethrough, task lists; callouts behind
  `ENABLE_GFM`), with the exceptions and degradations recorded in the
  element mapping below.
- The `typst` format's artifact is the emitted document itself; `pdf`
  compiles that identical document by running the `typst` binary. The
  compiler is not embedded as a crate dependency.
- **The emitted Typst is an intermediate representation.** Its job is
  to render into a clean PDF; reading or further processing it by hand
  is a non-goal, so the emitter always prefers the mechanically
  foolproof form (unconditional escaping, function calls) over
  idiomatic hand-written Typst. The `typst` output format exists for
  inspection and power users, without any readability promise.

## Escaping

Escaping is the correctness core of the emitter: note text is arbitrary,
and Typst markup assigns meaning to many plain characters. The rules below
derive from Typst's own lexer (`crates/typst-syntax/src/lexer.rs` in the
typst repository, main branch as of 2026-07-10), cross-checked against the
official syntax reference; they are not inferred from other converters.

### What Typst markup reacts to

Single characters, active anywhere in markup:

| Character | Meaning |
|---|---|
| `\` | escape introducer; before whitespace or end of line it is a forced line break |
| `#` | enters code mode |
| `[` `]` | content-block delimiters; an unbalanced `]` terminates an enclosing block early |
| `$` | opens math |
| `` ` `` | opens raw text |
| `*` `_` | strong/emphasis toggles (suppressed mid-word) |
| `<` | opens a label when followed by an identifier character |
| `@` | starts a reference when followed by an identifier character |
| `~` | non-breaking-space shorthand |
| `'` `"` | smart quotes (typographic substitution; no structural effect) |

Multi-character sequences, active anywhere:

- `...`, `--`, `---`, `-?` (ellipsis, dashes, soft hyphen), and `-` before
  a digit (minus sign),
- `http://` and `https://` (automatic link detection),
- `//` and `/* */` (line and block comments — live anywhere in markup,
  not only at line starts).

Line-anchored markers, recognized when followed by a space or line end:

- runs of `=` (heading), `-` (bullet item), `+` (numbered item),
  `/` (term-list item), and `<digits>.` (numbered item with an explicit
  start number).

The escape mechanism is universal: a backslash before any non-whitespace
character lexes as that literal character. Every problematic character is
neutralizable the same way; no character is unescapable.

### The three output contexts

Every character the emitter writes belongs to exactly one of three
contexts, each with its own fixed escaping rule. Every interpolation point
— body text, heading text, table cells, link labels and URLs, image paths,
captions, metadata values — is classified as one of them; there is no
third context and no unclassified splice.

**1. Markup text** (paragraph text, heading text, list items, table cells,
link labels, quote bodies, captions). Backslash-escape every occurrence of
each of these 20 characters:

    \  #  [  ]  $  `  *  _  <  >  @  ~  '  "  -  .  /  :  =  +

This set covers every trigger above: the always-active single characters
directly; the sequences via their members (`-` and `.` break the dash and
ellipsis shorthands, `/` breaks both comment forms, `:` and `/` break
`http://` autolinks); the line-anchored markers via `=`, `-`, `+`, `/`,
and the `.` in `<digits>.`. Escaping is unconditional — per occurrence,
with no line-start, word-boundary, or lookahead tracking — which is what
makes the rule foolproof: correctness never depends on emitter state.
Several members are escaped more often than strictly required (`-`, `.`,
`/`, `:`, `=`, `+`, and mid-word `*`/`_` are inert in many positions); the
cost is cosmetic noise in an intermediate artifact, never a rendering
difference.

Consequence, chosen deliberately: escaping `'`, `"`, `~`, `--`, `...`
disables Typst's typographic substitutions inside note text, so note
content renders character-for-character as written. Typography is a
styling concern and belongs to the theming layer, not to silent
rewriting of note text.

**2. String literals** (the quoted arguments of emitted function calls:
`#raw("...")`, `#link("...")`, image paths, any other string argument).
Escape exactly two characters: `\` becomes `\\` and `"` becomes `\"`.
The markup escape set is wrong in this context and must not be applied.

**3. Raw content** (fenced code blocks). No escaping; content passes
through verbatim between backtick fences. The fence length is computed
from the content: one backtick more than the longest backtick run inside
the block, with a minimum of three. The language tag follows the opening
fence directly.

### Verification

Escaping correctness is the round-trip property: Typst must read the
emitted markup back as exactly the original text. Tests assert it
in-process with the `typst-syntax` crate as a dev-dependency — the parser
half of Typst, no compiler, no I/O — keeping the existing policy that
tests execute no external tools (see Testing in
[rendering.md](rendering.md)). A round-trip test parses the emitted
markup, asserts it parses without errors, and reassembles the plain text
from `Text` and `Escape` nodes, requiring it to equal the input with no
other node kinds present. A typst-syntax version bump that changes the
markup surface therefore fails these tests instead of silently changing
rendered output.

The inputs are a curated corpus enumerated from the tables above: every
escape-set member at line start, mid-word, after a space, and at end of
input; every multi-character sequence; the backslash-before-whitespace
linebreak case; and the string-literal context's two characters. No
property-testing dependency is used. The corpus is also snapshot as
`input → escaped` pairs with `insta` (ADR 0021), so changes to the escape
set surface as reviewable snapshot diffs.

### Structural guarantee

The emitter never uses `*` or `_` as its own delimiters: span styling is
emitted as function calls (`#emph[...]`, `#strong[...]`, `#strike[...]`,
`#raw("...")`, `#link(...)[...]`). Since those characters are always
escaped in markup text, an emitted delimiter could otherwise pair with an
escaped occurrence in adjacent note text; function-call brackets have no
such interaction. Markup shorthand is reserved for constructs the emitter
anchors to line starts itself (headings, list items, code fences).

## Element mapping

How each Markdown construct materializes as Typst. Settled entries only;
the mapping grows as the bottom-up discussion settles further constructs.

| Markdown | Typst |
|---|---|
| Paragraph | blank-line separation (see below) |
| Heading level n | `=`×n, space, escaped text, newline (Markdown caps at 6; no clamping needed) |
| Emphasis / strong / strikethrough | `#emph[...]` / `#strong[...]` / `#strike[...]` |
| Inline code | `#raw("...")`, string-literal escaping — renders identically to backtick markup (visually verified, typst 0.15) |
| Code block | content-sized fence, language tag, verbatim body. The language tag is the info string's first whitespace-delimited token, and only when it is identifier-shaped (alphanumeric, `-`, `_`); anything else yields no tag, since the tag is spliced after the opening fence unescaped |
| Bullet list item | `- ` at line start |
| Ordered list item | `<n>. ` markers — markup carries explicit start numbers, so `6.` survives without `#enum(start:)` |
| Nested lists | child items indented under their parent, so Typst sees the nesting |
| Block quote | `#quote(block: true)[...]`, trailing newline |
| Callout (`> [!NOTE]` …) | `#callout(kind: "note")[...]`, defined by the prelude (see below) |
| Link (external URL) | `#link("url")[escaped label]` |
| Bare URL in text (`https://...`, `www....`) | detected by the emitter with the `linkify` crate over text events (GFM autolink rules; pulldown-cmark has no option for this — verified) and emitted as `#link("url")[url]`; a `www.` URL gains an `https://` scheme in the target |
| Email autolink (`<user@host>` or bare in text) | `#link("mailto:user@host")[user@host]`; the `mailto:` scheme is added so the link is actionable |
| Note link, resolved | `#emph[Title]`, the target note's current title emphasized |
| Note link, unresolved | the wrapper is dropped and the display text's inner inline events are re-emitted, so markup inside the display text survives |
| Task list item | bullet item with a `□ ` / `☒ ` lead-in — plain Unicode chosen for coverage in typst's default fonts (U+2610/U+2611 render as a missing glyph and an emoji there) |
| Footnote | `#footnote[...]` inlined at the reference site; definitions are buffered because pulldown-cmark delivers them separately from references |
| Thematic break | `#line(length: 100%)` |
| Soft / hard break | single space / `#linebreak()` |
| Table | `#table(columns: n, align: (...), table.header(...), cells)` |
| Image, local path | `#image("path")`, path verbatim (string-literal escaped) |
| Image, remote URL | `#link("url")[alt text]` plus a render warning — `typst compile` performs no network access, so remote images cannot appear in the artifact |
| Mermaid, emoji shortcodes, color chips | out of scope: rendered as their literal text (a mermaid block is an ordinary code block), no warning |
| Math (`$...$`, `$$...$$`, `math` fences) | not supported: `ENABLE_MATH` stays off, `$` is escaped like all text and renders literally, `math` fences render as plain code blocks |
| Raw HTML | dropped with a render warning naming what was dropped — never silently |

### Asset paths

Image paths and relative link targets pass through verbatim: the emitted
document behaves as if it sits in the same directory as the note. Typst
resolves relative paths against the file they appear in (not the process
working directory), so the `typst`-format artifact compiles correctly
when placed next to the note.

The pdf pipeline makes that premise hold via stdin: `typst compile -`
with the note's directory as the invocation's working directory, the
emitted document fed on stdin. Verified against typst 0.15.0: the stdin
document is treated as living at the project root, the root defaults to
the working directory, and relative paths resolve against it — so the
vault stays untouched and the pipeline compiles the `typst` format's
identical bytes.

The root is also typst's file-access sandbox, and the stdin document
always sits at the root, so note-relative resolution and access above
the note's directory are mutually exclusive. Consequence: assets
referenced with `../` above the note's directory are unsupported and
fail the compile with typst's explicit "would escape the project root"
error.

- **Paragraphs: blank-line separation**, not `#par()[...]`. Typst reads a
  blank line as a paragraph break natively. The escaping rules make the
  explicit wrapper's one advantage moot — soft breaks emit a space and
  every character of user text is escaped, so the emitter alone decides
  where blank lines appear — while the wrapper creates a real problem:
  block elements (images) inside `#par()` provoke "block element inside
  paragraph" warnings, which the reference implementation had to filter
  around by unwrapping image-only paragraphs. Blank-line separation makes
  that problem structurally impossible.
- **Callouts emit semantics, not appearance.** The five GFM callout kinds
  reach the emitter as kind-tagged block quotes (`ENABLE_GFM`) and are
  emitted as `#callout(kind: "...")[...]` so the kind survives into the
  artifact. The emitted document carries a **prelude** defining a default
  `callout` function; a theme replaces that definition
  (per-kind colored boxes via `#block(fill:, stroke:, radius:, inset:)`,
  per-kind symbols) without any emitter change. The prelude keeps the
  `typst` format's artifact self-contained and compilable on its own, and
  is the designated hook point for theming.

## Document skeleton

The emitted document is prelude, template application, then the
converted body — one emitted file serving both formats: the `typst`
format hands it out as the artifact, the `pdf` format compiles that
identical file.

```typst
// Prelude: engine-owned definitions; the theming hook.
#let callout(kind: "note", body) = ...
#let note(title: none, frontmatter: (:), body) = ...

#show: note.with(
  title: "My Note",
  frontmatter: (
    tags: ("area/work", "programming/rust"),
    created: "2026-07-10",
    ...
  ),
)

// Converted body.
```

- **Metadata travels as typed Typst values**, never as text spliced
  into markup: the complete frontmatter mapping is translated
  recursively — strings to string literals (string-literal escaping),
  integers/floats/booleans to their Typst counterparts, null to `none`,
  sequences to arrays, mappings to dictionaries. A metadata value can
  therefore never be interpreted as Typst code, whatever it contains.
- **The template receives all frontmatter fields** and the default
  `note` displays all of them: the title prominently, every other field
  as key-value lines beneath, nested values included. Which fields to
  feature or hide is a presentation decision and thus a theme's.
- **`#show: note.with(...)` is the single seam** between content and
  presentation. A theme is a different prelude defining `note` and
  `callout`; the document part never changes. The prelude is inlined,
  not imported, so the artifact stays a single self-contained file that
  compiles next to the note.
- `set document(title: ...)` inside `note` supplies the PDF metadata.
- The two formats never diverge: the pdf pipeline compiles the `typst`
  format's exact bytes, which the stdin-based asset mechanism (see
  Asset paths) preserves.

## Engine surface

The engine registers as `typst` (the value `--engine` accepts), is the
default engine for the formats `typst` (the emitted document as the
artifact) and `pdf` (that identical document compiled).

The pdf format's invocation feeds the document on stdin and controls
the working directory (see Asset paths). `Invocation` carries a stdin
payload and a working-directory field for this, and the fake context's
recorded transcript includes both, keeping the exact invocation
snapshot-testable without the tools installed. This follows ADR 0038's
rule that the context contract grows only when a real engine needs it.

## Writer and context model

Escaping is a property of the write call, not of tracked emitter state.
The output writer exposes one method per context — `markup_text`,
`string_literal`, `raw` — and the context choice is made lexically at
each call site. A fourth channel, `syntax`, carries the emitter's own
Typst markup (`#emph[`, `= `, fences, argument punctuation) unescaped; it
is module-private to the emitter, so the writer's public surface for
user-derived text is exactly the three escaped channels. There is no mode
flag to switch and therefore none to forget — the failure mode behind the
reference implementation's unescaped URLs and captions.

The event loop still carries a stack, but a structural one, needed
regardless of escaping: pulldown-cmark end events carry less data than
start events (URLs, table alignments, fence info, list types), so those
facts are saved at `Start` and consumed at `End`. `Event::Text` dispatches
on that stack's top: inside a code block to `raw`, everywhere else to
`markup_text`; `string_literal` is called only while emitting a specific
construct's quoted argument, never from the generic text arm.

The writer stays dumb about structure: block terminators, newline
discipline, and indentation are the event loop's job, because those rules
are per-construct knowledge and the writer must not grow a parallel model
of Markdown constructs.

