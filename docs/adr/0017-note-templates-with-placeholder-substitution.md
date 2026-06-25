# 17. Note templates with placeholder substitution

Date: 2026-06-24

## Status

Accepted

## Context

New notes are created from a template (ADR 0015). The template needs a
mechanism and a v1 scope.

## Decision

Templates are Markdown-with-frontmatter files in the per-vault
`.ntropy/templates/` directory. `new` uses `default.md` (with an embedded
fallback when it is absent); `new --template <name>` / `-t <name>` selects
`<name>.md` instead. A named template that does not exist is an error rather
than a silent fallback, so a misspelled name is caught. Names may not be empty
or contain a path separator, so selection cannot escape the templates
directory.

Substitution is hand-rolled: ntropy replaces a fixed placeholder set, no
template-engine dependency. Placeholders: `{{title}}`, `{{id}}` (ULID),
`{{date}}` (locally rendered creation date), `{{slug}}`.

## Consequences

- No template-engine dependency; substitution is predictable.
- Templates have no conditionals or loops.
- Note types are expressed as additional template files; a richer engine can be
  added later without changing the template file location.
