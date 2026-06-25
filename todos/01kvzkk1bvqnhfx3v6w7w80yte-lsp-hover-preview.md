# LSP hover preview via textDocument/hover (tier 2)

Deferred tier-2 ntropy language-server feature.

On hovering an ntropy link, resolve its ULID (ADR 0028) and show the target
note's title and a short body excerpt without opening it. Reuses link
resolution and the in-memory session scan cache.

## Open questions

- Excerpt extraction: first paragraph after frontmatter vs. first N lines.
</content>
