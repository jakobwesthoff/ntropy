# Site: math and diagram rendering

Deferred from the HTML engine (ADR 0049, `docs/design/html-engine.md`).
Decision (user, 2026-09-11): neither in the first implementation, parity
with the Typst engine. Math stays off in the parser and renders as literal
text; a `mermaid` fence is an ordinary code block.

The Typst side has its own deferred math todo
(`01kx5n2ww5526gtfmhga2b8xe4-typst-engine-math-support-via-mitex.md`).

## What was discussed

- KaTeX for math, shipped inside the exported site and run in the
  browser, which needs `ENABLE_MATH` in pulldown-cmark for the HTML path
  and diverges from the Typst engine as long as that side stays off.
- Mermaid for diagrams, shipped inside the site and run in the browser; a
  large library.
- Both have to be vendored into the export: the site works over `file://`
  with no network (ADR 0046).

## To decide later

- Whether math is enabled for both engines at once, to keep the parser
  options shared by the Markdown walk (ADR 0049) identical.
- The size budget: Mermaid and KaTeX would be loaded per page like the
  Shiki grammars (ADR 0053), only where a page uses them.
