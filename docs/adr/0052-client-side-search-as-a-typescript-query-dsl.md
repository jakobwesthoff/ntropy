# 52. Client-side search as a TypeScript reimplementation of the query DSL

Date: 2026-09-11

## Status

Accepted

Gives the site of
[ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md)
the search of [ADR 0012](0012-query-dsl-with-hand-rolled-parser.md) and
[ADR 0030](0030-replace-ripgrep-stack-with-regex-crate-for-full-text-search.md),
written in the toolchain of
[ADR 0051](0051-browser-side-code-in-typescript-with-committed-build.md).

## Context

The CLI's interactive search has two layers: a DSL query filters the note
set, then the picker fuzzy-matches over rows of id, title, date, tags,
and path. The query module is 1181 lines of Rust, evaluates against an
in-memory note with no I/O, and depends on the crate's `note` and `text`
modules and on `serde_yaml_ng`. The crate as a whole also depends on
`crossterm`, `ignore`, `tempfile`, and `libc`, so it does not build for
`wasm32` as it stands.

Every surveyed JavaScript search library (MiniSearch, FlexSearch, Fuse.js,
Lunr, Orama, Elasticlunr) is a tokenizing full-text index with its own
query syntax; none evaluates regexes over bodies and none implements the
ntropy DSL. Pagefind loads its index by `fetch` and its maintainer states
it does not work over `file://`.

Rust `regex` and JavaScript `RegExp` differ: `\pL` versus `\p{L}` with the
`u` flag, inline flags, lookaround (JavaScript accepts what Rust rejects),
`\b` (Unicode-aware in Rust, ASCII in JavaScript), and line terminators.

## Decision

The query DSL is reimplemented in TypeScript and evaluated in the browser
over embedded note data: id, title, tags, frontmatter, body.

- `tag:`, `field:`, `and`, `or`, `not`, and parentheses mirror the Rust
  semantics.
- `text:` maps to JavaScript `RegExp`. A pattern using a construct Rust
  `regex` rejects, lookaround and backreferences among them, is refused
  with a message naming the construct, so a query that works on the site
  also works in the CLI.
- One hand-maintained JSON file under `tests/fixtures/` holds conformance
  cases: a query, note fixtures as frontmatter plus body, and the expected
  matching ids or expected error. The Rust tests and the Vitest tests both
  load it.
- A fuzzy narrowing layer over titles and tags, the picker's rows, is
  written in TypeScript.

No JavaScript search library and no WebAssembly.

### Rejected alternatives

- **A WebAssembly build of the query module.** Exact semantics from one
  implementation, at the cost of making the query module buildable for
  `wasm32` (a workspace split or feature gating), the `wasm32` target and
  `wasm-bindgen` with its version-locked CLI, and a module of roughly 200
  to 600 KiB embedded as base64 for `file://`. Its `no-modules` target is
  heading for deprecation.
- **A JavaScript search library**, alone or as an instant box beside the
  DSL. A different search than the CLI's.

## Consequences

- Two implementations of the DSL exist, kept in step by the shared
  corpus.
- The rejection rule covers constructs Rust rejects; a pattern both
  engines accept can still match differently where their semantics
  differ, as listed in the context.
- The search data embeds every exported note's body.
