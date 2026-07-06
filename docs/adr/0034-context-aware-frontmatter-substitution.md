# 34. Context-aware frontmatter substitution

Date: 2026-07-06

## Status

Accepted

Supersedes ADR 0017's verbatim-substitution contract for the template's
frontmatter region only. The placeholder set, the templates directory
layout, and body substitution are unchanged.

## Context

Template substitution (ADR 0017) is a single verbatim pass: a recognized
placeholder is replaced with its value, unconditionally. The default
template's frontmatter line `title: {{title}}` puts the substituted title
directly into YAML syntax, so a title such as `Q3: Planning kickoff` (a bare
`: ` inside a plain scalar) or `[draft] roadmap` (a leading flow-sequence
character) breaks the line's YAML rather than being taken as plain text, and
`new` fails.

## Decision

Substitution becomes context-aware within the frontmatter region only. The
template is split into a frontmatter region (between the leading `---` line
and its closing delimiter, using the same delimiter rules as
`note::frontmatter::split`) and the body. Body substitution stays verbatim.
Within the frontmatter region, each placeholder occurrence is classified by
the YAML syntax already surrounding it on its line, and the classification
applies uniformly to every placeholder (`title`, `id`, `date`, `slug`):

| Template context | Example | Treatment |
|---|---|---|
| Bare, whole value: the placeholder is the entire scalar value of a `key:` mapping line (`^\s*<key>:\s*\{\{name\}\}\s*$`) | `title: {{title}}` | Replace with a YAML-safe scalar: plain (unquoted) when provably safe, otherwise quoted and escaped |
| Inside a double-quoted string on the line, whole or embedded | `title: "{{title}}"`, `title: "Meeting {{title}}"` | Escape the value for double-quote style: `\` → `\\`, `"` → `\"`, newline → `\n`, other control chars escaped |
| Inside a single-quoted string on the line, whole or embedded | `title: '{{title}}'` | Double every `'` in the value (`it's` → `it''s`) |
| Anything else (embedded in a bare plain value, flow collections like `tags: [x, {{slug}}]`, etc.) | `title: Meeting {{title}} notes` | Verbatim, exactly as before |

Quote-context detection scans the line from its start, tracking YAML quote
state (single quotes toggle on `'` with `''` as the escape; double quotes
toggle on `"` with `\"` as the escape); the state at the placeholder's
position decides the row. The bare-whole-value scalar is produced by
serializing the value with `serde_yaml_ng::to_string` and trimming its
trailing newline, delegating the plain-vs-quoted judgment to the same YAML
implementation `note::frontmatter::parse_block` reads with, rather than
reimplementing YAML's plain-scalar grammar by hand. A value containing a
newline or another control character is instead emitted as a single-line
double-quoted escaped string, since its serde serialization would be a
multi-line block scalar spliced into a one-line field.

An unrecognized placeholder and an unterminated `{{` stay verbatim
everywhere, including the frontmatter region. A template with no frontmatter
block renders entirely verbatim.

Substitution never sanitizes or alters the value: escaping is lossless, so
the stored title, once parsed back out of the rendered frontmatter, always
equals the title as typed. The one context with no lossless escape available
— a placeholder embedded in a bare plain scalar, or inside a flow collection
— is left verbatim rather than silently altered; a separate
validate-before-write decision is responsible for catching breakage that
still slips through that row.

## Consequences

- A title containing `: `, or starting with `[`, `#`, or another YAML syntax
  character, now creates successfully through the default template instead of
  producing invalid YAML.
- The bare plain-scalar and flow-collection contexts still have no lossless
  escape and can still produce invalid YAML; those are left to fail loudly
  under a later validate-before-write check rather than being masked here.
