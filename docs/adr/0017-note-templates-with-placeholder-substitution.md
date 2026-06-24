# 17. Note templates with placeholder substitution

Date: 2026-06-24

## Status

Accepted

## Context

New notes are created from a template (ADR 0015). The template needs a
mechanism and a v1 scope.

## Decision

Templates are Markdown-with-frontmatter files in the per-vault
`.ntropy/templates/` directory. v1 uses a single default template.

Substitution is hand-rolled: ntropy replaces a fixed placeholder set, no
template-engine dependency. v1 placeholders: `{{title}}`, `{{id}}` (ULID),
`{{date}}` (locally rendered creation date), `{{slug}}`.

Named templates / note types are deferred.

## Consequences

- No template-engine dependency; substitution is predictable.
- Templates have no conditionals or loops in v1.
- A richer engine or named templates can be added later without changing the
  template file location.
