---
title: "Support math in the Typst engine through mitex"
kind: feature
component: render
horizon: someday
origin: discussion
---
# Support math in the Typst engine through mitex

Math is parked in the custom Typst render engine. Passing LaTeX through to
Typst does not work, so support needs a LaTeX-to-Typst translation step. This
todo keeps the findings and the build path, so the feature can be picked up
without repeating the research. The support target is what GitHub renders,
not the formal GFM spec.

## Decisions

- 2026-07-10, user: math is not supported for now.
- 2026-07-10, user: the support target is what GitHub renders, not the formal GFM spec.

## Findings

### What GitHub supports

GitHub renders LaTeX math (MathJax) in four wrappers:

- inline `$...$` (delimiter rules avoid currency: no space directly inside
  the dollars, no digit directly after the closing one),
- display `$$...$$`,
- fenced code blocks with `math` as the language tag,
- the collision-avoiding inline form `` $`...`$ ``.

The expression language in all four is TeX math as MathJax implements it:
backslash commands with brace and optional arguments (`\frac{a}{b}`,
`\sqrt[3]{x}`, `\alpha`, `\mathbb{R}`), environments (`pmatrix`, `aligned`,
`cases`), `_` and `^` scripts, a symbol vocabulary of several hundred
commands, and `\newcommand` macros.

### What Typst supports

Typst math is first-class but a different language, not a TeX dialect.
Delimiters: `$x^2$` (content touching the dollars) is inline, `$ x^2 $`
(spaces inside) is display. Syntax differences run through everything:

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
(minding grouping and operator binding), and serialize as Typst math with
inline or display spacing chosen by the source wrapper.

### The existing translator: mitex

`mitex` (<https://github.com/mitex-rs/mitex>, crates.io `mitex`,
Apache-2.0, actively developed as of 2026-07-10) is this translator: LaTeX
to AST to Typst code. Coverage includes user-defined macros, equation
environments (aligned, matrices, cases), references, and coloring; package
support is on its roadmap. It is small (about 185 KB) and fast (its
benchmark: 32.5k equations in about 2.3 s as WASM). Its gaps are the long
tail of LaTeX package-specific commands.

## Build path

1. Enable `ENABLE_MATH` in pulldown-cmark. This yields `InlineMath` and
   `DisplayMath` events carrying the raw LaTeX string. It covers `$...$` and
   `$$...$$` but not the `` $`...`$ `` variant. Fenced `math` blocks arrive as
   ordinary code blocks with language tag `math`; special-case them into the
   math path instead of the raw path.
2. Add the `mitex` dependency with `cargo add`. Feed each math event's LaTeX
   through mitex, and emit the returned Typst code through the writer's
   `syntax` channel, wrapped in Typst math delimiters (touching dollars for
   inline, spaced for display). The output is trusted generated Typst, not
   user text, so it bypasses escaping by design.
3. On a mitex conversion failure, fall back to the original source as escaped
   literal text plus a render warning. This matches the engine's raw-HTML
   policy: degraded output, never a failed compile, never a silent drop. The
   worst case for an exotic expression equals the unsupported behavior.

## Interim behavior

Without math support the escaping design already guarantees correct output:
`$` is in the markup escape set, so no note text can open Typst math by
accident. Math source renders as literal text, and fenced `math` blocks
render as plain code blocks.

## Revisit when

No trigger known; reconsider at the next sweep.

## Relations

- Relates to: [Finish the remaining work on the custom Typst render engine](01KX5HARVK9NGCXS7Q63W8GE18-finish-the-remaining-work-on-the-custom-typst-render-engine.md), the math part of that engine's remaining work
