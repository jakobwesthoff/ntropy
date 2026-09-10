# 46. Static site export with a `site` command and an `html` render format

Date: 2026-09-11

## Status

Accepted

Extends the command surface of [ADR 0018](0018-cli-command-surface.md) as
amended by [ADR 0037](0037-render-command-surface.md) with a second
artifact-producing command, and adds a format to the registry of
[ADR 0038](0038-pluggable-rendering-engine-with-pandoc-and-typst.md).
Interactivity and the `-p` flag follow
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md). The
site's structure is [ADR 0054](0054-site-navigation-and-url-scheme.md), its
themes [ADR 0047](0047-themes-directory-split-by-type.md) and
[ADR 0048](0048-site-themes-as-stylesheets-and-assets.md), its converter
[ADR 0049](0049-shared-markdown-walk-with-typst-and-html-emitters.md), its
templates [ADR 0050](0050-page-templates-with-minijinja.md), its browser
code [ADR 0051](0051-browser-side-code-in-typescript-with-committed-build.md),
its search [ADR 0052](0052-client-side-search-as-a-typescript-query-dsl.md),
and its highlighting [ADR 0053](0053-syntax-highlighting-with-shiki-in-the-browser.md).

## Context

`render` produces one artifact from one note per invocation, and both of
its formats are print-oriented. Nothing turns a vault into something a
browser shows, so publishing notes as a documentation site means leaving
ntropy. The ways ntropy offers to reach a note, the chronological list,
tags, the configured views, and the query DSL, exist only on the
filesystem and in the terminal.

The request sets two constraints. The output is a set of static files that
any file host serves with no ntropy-specific server, and it works when
opened straight from disk, search included. A desktop application is
planned for later and should be able to reuse rendering logic; nothing is
built for it now.

## Decision

### Architecture

The export is a classic static-site generator: Rust pre-renders every page
at export time. Browser-side JavaScript adds search, the heading outline,
the light/dark toggle, and syntax highlighting; it never constructs pages.

The site works over `file://`. Browsers restrict `fetch()` of local files
and module scripts there, so data the browser code needs ships embedded
in classic script files, every page is a real `.html` file, links are
relative, and there is no client-side routing.

### Command

    ntropy site -o <dir> [query] [--theme <name>] [--force] [-p]

- `-o` is required; there is no default location. A non-empty directory
  is refused unless `--force` is given, in which case it is emptied before
  writing.
- The optional query is a DSL query that restricts the exported notes. The
  default is every note. A link to a note outside the set renders as
  unresolved.
- `--theme` overrides the configured site theme (ADR 0048).
- `-p` prints the path of the entry page, `index.html`, as one line, so
  `open "$(ntropy site -o dir -p)"` composes. Without it a completion
  report names the directory, the page count, and the warnings.
- Scan warnings and export warnings (a referenced file that is missing, a
  fence language without a grammar, a link to a note outside the set)
  print to stderr. Under `--strict` both kinds fail the exit code. The
  site is written either way. This is `render`'s rule.
- An encrypted vault exports like a plaintext one. When the output
  directory lies inside the vault, a warning is printed, as `render` does
  for an artifact.
- The export runs once and exits. There is no watch mode and no preview
  server.

`ntropy site theme init <name>` is defined in ADR 0048.

### The `html` format on `render`

`render --to html` registers `html` in the format registry, produced by
the same converter as the site (ADR 0049). The artifact is one
self-contained file: the theme's stylesheet inlined, the note content and
its frontmatter block, and note links targeting `<slug>.html` beside the
artifact, the HTML counterpart of the `<slug>.pdf` convention of
[ADR 0044](0044-cross-document-links-between-rendered-notes.md). No
sidebar, no search.

### Rejected alternatives

- **A viewer application fed by exported data** (HTML fragments plus JSON
  manifests, a browser-side viewer rendering navigation and search,
  pre-rendered pages as entry points), and **a pure client-side app**
  with no pre-rendered pages. The pure client app is unreadable without
  JavaScript.
- **Growing `render` into a multi-note command.** Contradicts the
  one-note-per-invocation rule of ADR 0037.
- **Requiring a static HTTP host.** Would allow fetched JSON and clean
  URLs.
- **`export`, `publish`, `web`** as the command name.
- **A default output directory, and overwriting in place** without
  deleting.
- **A `--watch` mode and a built-in preview server.**

## Consequences

- The command surface grows by one command that produces a directory
  rather than a file, and `render` gains a third format.
- The site writes a tree of files, which the single-artifact
  `RenderContext` of ADR 0038 does not cover.
- The search data embeds every exported note's body (ADR 0052), so a
  published site carries each note twice, as its page and inside the
  search data.
- An exported site of an encrypted vault is plaintext, its directory
  names and page structure included.
