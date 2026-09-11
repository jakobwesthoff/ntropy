# 51. Browser-side code in TypeScript with a committed build

Date: 2026-09-11

## Status

Accepted

Amended 2026-09-11: Bun is the JavaScript runtime and package manager for
the toolchain; Node is not used. The user's rule is "bun over node,
always".

Amended 2026-09-11 by
[ADR 0055](0055-theme-directory-layout-with-fonts-icons-and-embedded-assets.md):
the stylesheet lives in the built-in theme under `src/site/theme/`, not
in `site/`; `src/site/dist/` is embedded by a build script rather than
by hand-written includes; the theme's font and icon files are third
party and carry their own licenses instead of the MPL-2.0 header.

Provides the browser-side code of
[ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md).
Search is [ADR 0052](0052-client-side-search-as-a-typescript-query-dsl.md),
highlighting [ADR 0053](0053-syntax-highlighting-with-shiki-in-the-browser.md).
Distribution stays as in
[ADR 0022](0022-distribute-via-crates-io-for-v1-under-mpl-2-0.md).

## Context

The site needs code that runs in the browser: search, the heading
outline, the light/dark toggle, syntax highlighting. ntropy is one binary
built from the crates.io tarball with `cargo` alone, and nothing in the
build runs a JavaScript toolchain. Shiki, the chosen highlighter, ships
ESM-only with no classic-script build, and the site must work over
`file://`, where module scripts do not load.

## Decision

### Language and tools

Browser-side code is TypeScript with Preact, built with Vite, tested with
Vitest, linted and formatted with Biome. Bun is the runtime and package
manager that installs dependencies and runs all three; Node is not used
anywhere in the toolchain.

### Repository layout

Sources (package manifest, TypeScript, CSS, tests) live in `site/` at the
repository root. Vite writes the built output to `src/site/dist/`, which
is committed and embedded into the binary. Every output file is a classic
script or stylesheet; there are no module scripts and no lazily loaded
chunks. `cargo build` never invokes Bun.

A CI job builds `site/` with Bun and fails when the result differs from
the committed `src/site/dist/`. A `just` recipe runs the same check
locally.

### License headers

TypeScript, CSS, and configuration files under `site/` carry the MPL-2.0
header in their comment syntax. Files under `src/site/dist/` are generated
and carry none, the rule that applies to the seed content of
[ADR 0039](0039-vault-seed-content-as-embedded-files.md). A test enforces
both.

### Rejected alternatives

- **Vanilla JavaScript and CSS with no toolchain.**
- **A separate prebuilt frontend crate** the main crate depends on.
- **esbuild with Node's test runner**, and **Bun's own bundler and test
  runner** in place of Vite and Vitest.
- **oxlint with a separate formatter**, **ESLint with typescript-eslint
  and Prettier**, and **no linter** in place of Biome.
- **Solid** as the framework, and **no framework**.
- **`web/` as the source directory**, and `src/site/assets/` or
  `src/site/build/` as the output directory.
- **Headers on generated output too**, and **headers by convention
  without a test**.

## Consequences

- Changing the browser-side code needs Bun; building ntropy does not.
- The repository and the crate tarball carry generated files, kept
  honest by the CI diff.
- Preact's component model is the shape of the search interface.
