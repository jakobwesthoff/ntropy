---
title: "Render math and diagrams on site note pages"
kind: feature
component: site
status: needs-discussion
origin: discussion
---
# Render math and diagrams on site note pages

Site note pages should render LaTeX math and Mermaid diagrams. Neither is in
the first implementation of the HTML engine (ADR 0049, `docs/design/html-engine.md`),
which keeps parity with the Typst engine. Math stays off in the parser and
renders as literal text, and a `mermaid` fence is an ordinary code block.

## Decisions

- 2026-09-11, user: neither math nor diagrams in the first implementation, for parity with the Typst engine.

## Proposal

- KaTeX for math, shipped inside the exported site and run in the browser.
  This needs `ENABLE_MATH` in pulldown-cmark for the HTML path, and it
  diverges from the Typst engine for as long as that side stays off.
- Mermaid for diagrams, shipped inside the site and run in the browser. It is
  a large library.
- Both have to be vendored into the export, because the site works over
  `file://` with no network (ADR 0046).

## Open questions

- Whether math is enabled for both engines at once, to keep the parser options
  shared by the Markdown walk (ADR 0049) identical.
- The size budget. Mermaid and KaTeX would load per page like the Shiki
  grammars (ADR 0053), and only on pages that use them.

## Relations

- Relates to: [Support math in the Typst engine through mitex](01KX5N2WW5526GTFMHGA2B8XE4-support-math-in-the-typst-engine-through-mitex.md), the same math build path on the Typst side
