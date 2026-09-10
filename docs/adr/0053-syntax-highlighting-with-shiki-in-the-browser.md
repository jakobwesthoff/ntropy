# 53. Syntax highlighting with Shiki in the browser

Date: 2026-09-11

## Status

Accepted

Highlights the code blocks the converter of
[ADR 0049](0049-shared-markdown-walk-with-typst-and-html-emitters.md) marks
with their language, using the toolchain of
[ADR 0051](0051-browser-side-code-in-typescript-with-committed-build.md).

## Context

Code blocks reach the page as `<pre><code>` with a language. Highlighting
can happen in Rust at export time or in the browser. Shiki 4.4.3 is
published ESM-only; its JavaScript regex engine needs no WebAssembly and
supports every built-in grammar; `createHighlighterCore` takes explicit
grammar and theme objects and `loadLanguage` adds grammars later. Measured
from the published files, the core is 50 KB, the Rust grammar 17 KB, the
TypeScript grammar 191 KB. Shiki's own `web` bundle is 3.8 MB minified and
does not contain Rust.

The site cannot load chunks lazily over `file://`, so the grammars a page
can use are fixed when the binary is built.

## Decision

Code blocks are highlighted in the browser by Shiki with the JavaScript
regex engine, shipped inside the exported site. Code is plain text until
the scripts run.

A curated grammar set is embedded in the binary: Shiki's web bundle
languages plus rust, go, python, ruby, java, kotlin, swift, c, cpp,
csharp, shellscript, powershell, toml, ini, dockerfile, makefile, sql,
diff, lua, typst, latex. Vite builds one classic script file per grammar.
Each exported page includes only the grammar scripts for the fence
languages it contains. A fence language outside the set renders as plain
code and produces an export warning.

Shiki has no classic-script build; the per-grammar files and their
registration with the highlighter are ntropy's own construction.

### Rejected alternatives

- **syntect in Rust at export time.** Static HTML with CSS classes, no
  JavaScript needed to read code.
- **Shiki at export time through Node.** Every user exporting a site would
  need Node installed.
- **Shiki's `web` bundle only** (no Rust), **Shiki's full set** (the
  languages package is 8.6 MB unpacked), **one bundle with the whole set
  on every page**, and **a config list restricting the embedded set**.
- **Vault-supplied grammar files** registered by the export.

## Consequences

- A page's weight in scripts follows the languages it uses; a page
  without code blocks loads no grammar.
- Adding a language means editing the curated list and rebuilding the
  frontend; no vault can add one.
- Without JavaScript the code is readable but unhighlighted.
