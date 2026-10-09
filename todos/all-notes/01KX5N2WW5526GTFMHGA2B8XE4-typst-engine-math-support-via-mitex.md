# Typst engine: math support via mitex

Deferred feature of the custom typst render engine
(`01kx5harvk9ngcxs7q63w8ge18-custom-typst-render-engine.md`,
`docs/design/typst-engine.md`). Decision (user, 2026-07-10): math is not
supported for now. This todo preserves the findings and the build path so
the feature can be picked up without re-research. The support target for
the engine is what GitHub renders, not the formal GFM spec (user,
2026-07-10).

## Findings

### What GitHub supports

GitHub renders LaTeX math (MathJax) in four wrappers:

- inline `$...$` (delimiter rules avoid currency: no space directly
  inside the dollars, no digit directly after the closing one),
- display `$$...$$`,
- fenced code blocks with `math` as the language tag,
- the collision-avoiding inline form `` $`...`$ ``.

The expression language in all four is TeX math as MathJax implements
it: backslash commands with brace/optional arguments (`\frac{a}{b}`,
`\sqrt[3]{x}`, `\alpha`, `\mathbb{R}`), environments (`pmatrix`,
`aligned`, `cases`), `_`/`^` scripts, a symbol vocabulary of several
hundred commands, and `\newcommand` macros.

### What Typst supports

Typst math is first-class but a different language, not a TeX dialect.
Delimiters: `$x^2$` (content touching the dollars) is inline,
`$ x^2 $` (spaces inside) is display. Syntax differences run through
everything:

| LaTeX (GitHub) | Typst |
|---|---|
| `\frac{a}{b}` | `a / b` or `frac(a, b)` |
| `\alpha`, `\cdot`, `\infty` | `alpha`, `dot.op`, `infinity` |
| `\sqrt[3]{x}` | `root(3, x)` |
| `\sum_{i=1}^{n}` | `sum_(i=1)^n` |
| `\mathbb{R}` | `RR` or `bb(R)` |
| `\begin{pmatrix}...\end{pmatrix}` | `mat(...)` |
| `\text{if } x` | `"if " x` |

Passthrough is therefore ruled out: anything beyond the trivial overlap
(`x^2`) is garbage or a compile error in Typst math. Real support means
parse-and-map translation: parse LaTeX to an AST (commands, arguments,
environments, macro expansion), map every node to its Typst counterpart
(minding grouping and operator binding), serialize as Typst math with
inline/display spacing chosen by the source wrapper.

### The existing translator: mitex

`mitex` (<https://github.com/mitex-rs/mitex>, crates.io `mitex`,
Apache-2.0, actively developed as of 2026-07-10) is exactly this
translator: LaTeX to AST to Typst code. Coverage includes user-defined
macros, equation environments (aligned, matrices, cases), references,
and coloring; package support is on its roadmap. Small (~185 KB) and
fast (their benchmark: 32.5k equations in ~2.3 s as WASM). Its gaps are
the long tail of LaTeX package-specific commands.

## Build path

1. Enable `ENABLE_MATH` in pulldown-cmark. This yields `InlineMath` /
   `DisplayMath` events carrying the raw LaTeX string; it covers
   `$...$` and `$$...$$` but not the `` $`...`$ `` variant. Fenced
   `math` blocks arrive as ordinary code blocks with language tag
   `math`; special-case them into the math path instead of the raw
   path.
2. Add the `mitex` dependency (via `cargo add`). Feed each math event's
   LaTeX through mitex; emit the returned Typst code through the
   writer's `syntax` channel wrapped in Typst math delimiters (touching
   dollars inline, spaced display). The output is trusted generated
   Typst, not user text, so it bypasses escaping by design.
3. On a mitex conversion failure, fall back to the original source as
   escaped literal text plus a render warning — consistent with the
   engine's raw-HTML policy: degraded output, never a failed compile,
   never a silent drop. Worst case for an exotic expression equals the
   unsupported behavior.

## Interim behavior until built

Without math support the escaping design already guarantees correct
output: `$` is in the markup escape set, so no note text can ever open
Typst math accidentally; math source renders as literal text, and
fenced `math` blocks render as plain code blocks.
