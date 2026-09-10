# Site themes: template override

Deferred from the site theme model (ADR 0048, `docs/design/site-export.md`).
A site theme is stylesheets and static assets over an HTML structure that
ntropy's embedded minijinja templates produce (ADR 0050). Decision (user,
2026-09-10): "css and assets only for the first implementation maybe
templates in a second iteration later on".

## What was discussed

- A theme directory could carry page templates rendered by the same
  engine, giving a theme control over markup, layout, and navigation.
- The layered variant: built-in templates stay, a theme overrides
  individual ones and inherits the rest.
- minijinja's `path_loader` loads templates from a directory, so the
  engine already has the loading half; the missing half is the contract.

## To decide later

- The template data model as a documented contract: which context fields
  a template receives per page kind, and how changes to it are versioned.
- Whether a theme overrides whole templates or named blocks.
- Whether `site theme init` writes the built-in templates alongside the
  stylesheets once templates are overridable.
