# 50. Page templates with minijinja

Date: 2026-09-11

## Status

Accepted

Produces the page structure of
[ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md)
that themes of [ADR 0048](0048-site-themes-as-stylesheets-and-assets.md)
style.

## Context

Every page of the site wraps a converted note body or a generated listing
in the same structure: sidebar, breadcrumb, outline, previous and next
links, frontmatter block. Themes do not supply templates (ADR 0048), so
the structure is ntropy's own and lives in the binary.

## Decision

Pages are assembled by minijinja from templates embedded in the binary as
strings. The page data is handed to the templates as a serde-serializable
context.

minijinja 2.24.0 (Apache-2.0) has two required dependencies, `serde` and
`memo-map`; its stated goals are a compact API, minimal dependencies, and
staying close to Jinja2. Inheritance, `include`, and `import` are provided
by its `multi_template` feature; `path_loader` loads templates from a
directory.

### Rejected alternatives

- **tera 2.3.0** (MIT). Its feature set was not examined; the 1.x line is
  what Zola uses.
- **Compile-time templates**, askama or maud. Typed and checked at build
  time; a later vault-side template override would need a second
  mechanism.
- **Hand-written string assembly** with an escaping helper, the way the
  Typst engine assembles its document.

## Consequences

- One runtime template engine joins the dependencies.
- Templates are files edited as HTML, embedded like the seed content of
  [ADR 0039](0039-vault-seed-content-as-embedded-files.md).
- The template data model is an explicit context type rather than code
  paths that build strings.
