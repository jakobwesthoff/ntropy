---
title: Design
tags: [docs/develop]
site:
  order: 2
---
The full design is recorded as decision records under
[`docs/adr/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/adr)
and narrative documents under
[`docs/design/`](https://github.com/jakobwesthoff/ntropy/tree/main/docs/design).
A design document describes how one part of ntropy works today. A decision
record holds one choice behind that, dated, in the order the choices were
made.

## Design documents

- [CLI reference (v1)](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/cli.md)
- [Configuration](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/configuration.md)
- [Encryption](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/encryption.md)
- [The HTML engine](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/html-engine.md)
- [Language server](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/language-server.md)
- [Query and search](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/query-and-search.md)
- [Rendering](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/rendering.md)
- [Shell integration](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/shell-integration.md)
- [Site export](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/site-export.md)
- [Site frontend](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/site-frontend.md)
- [The typst engine](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/typst-engine.md)
- [Vault layout and views](https://github.com/jakobwesthoff/ntropy/blob/main/docs/design/vault-layout-and-views.md)

## Decision records

- [0001. Record architecture decisions](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0001-record-architecture-decisions.md)
- [0002. Stateless filesystem scanning over a derived index](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0002-stateless-filesystem-scanning-over-a-derived-index.md)
- [0003. Flat single-vault storage layout](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0003-flat-single-vault-storage-layout.md)
- [0004. Note identity and filename strategy](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0004-note-identity-and-filename-strategy.md)
- [0005. Permissive frontmatter schema with recognized fields](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0005-permissive-frontmatter-schema-with-recognized-fields.md)
- [0006. Hierarchical tags by slash convention](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0006-hierarchical-tags-by-slash-convention.md)
- [0007. Vault directory layout](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0007-vault-directory-layout.md)
- [0008. Materialized symlink views](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0008-materialized-symlink-views.md)
- [0009. Generic group-by-field view definitions](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0009-generic-group-by-field-view-definitions.md)
- [0010. Render derived dates in system local timezone](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0010-render-derived-dates-in-system-local-timezone.md)
- [0011. Embed ripgrep libraries for full-text search](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0011-embed-ripgrep-libraries-for-full-text-search.md)
- [0012. Query DSL with hand-rolled parser](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0012-query-dsl-with-hand-rolled-parser.md)
- [0013. Library/binary split with thiserror and anyhow](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0013-library-binary-split-with-thiserror-and-anyhow.md)
- [0014. Interactive-by-default CLI with auto output mode](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0014-interactive-by-default-cli-with-auto-output-mode.md)
- [0015. Editor integration and new-note flow](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0015-editor-integration-and-new-note-flow.md)
- [0016. Configuration format, location and vault resolution](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0016-configuration-format-location-and-vault-resolution.md)
- [0017. Note templates with placeholder substitution](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0017-note-templates-with-placeholder-substitution.md)
- [0018. CLI command surface](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0018-cli-command-surface.md)
- [0019. Scan robustness and resource tolerance](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0019-scan-robustness-and-resource-tolerance.md)
- [0020. Unix-only v1 with soft performance target](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0020-unix-only-v1-with-soft-performance-target.md)
- [0021. Testing strategy with insta across all layers](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0021-testing-strategy-with-insta-across-all-layers.md)
- [0022. Distribute via crates.io for v1 under MPL-2.0](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0022-distribute-via-crates-io-for-v1-under-mpl-2-0.md)
- [0023. Slug, tag and disambiguator normalization rules](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0023-slug-tag-and-disambiguator-normalization-rules.md)
- [0024. v1 dependency selection](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0024-v1-dependency-selection.md)
- [0025. Plain output format, ordering and edit ambiguity](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0025-plain-output-format-ordering-and-edit-ambiguity.md)
- [0026. Project-local vault pointer file](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0026-project-local-vault-pointer-file.md)
- [0027. In-house fuzzy picker over nucleo and crossterm](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md)
- [0028. Note-to-note links as standard Markdown links](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0028-note-to-note-links-as-standard-markdown-links.md)
- [0029. Language server over lsp-server with a session scan cache](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0029-language-server-over-lsp-server-with-a-session-scan-cache.md)
- [0030. Replace the ripgrep stack with the regex crate for full-text search](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0030-replace-ripgrep-stack-with-regex-crate-for-full-text-search.md)
- [0031. Merge edit into search](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0031-merge-edit-into-search.md)
- [0032. Auto-managed .gitignore for view directories](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0032-auto-managed-gitignore-for-views.md)
- [0033. Aligned plain-table output](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0033-aligned-plain-table-output.md)
- [0034. Context-aware frontmatter substitution](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0034-context-aware-frontmatter-substitution.md)
- [0035. Generic --print flag replaces --no-edit](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0035-generic-print-flag-replaces-no-edit.md)
- [0036. Interactivity keyed to the controlling terminal](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0036-interactivity-keyed-to-the-controlling-terminal.md)
- [0037. Render command surface](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0037-render-command-surface.md)
- [0038. Pluggable rendering engine with pandoc and typst](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0038-pluggable-rendering-engine-with-pandoc-and-typst.md)
- [0039. Vault seed content as embedded files](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0039-vault-seed-content-as-embedded-files.md)
- [0040. Custom typst engine with own Markdown emitter](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0040-custom-typst-engine-with-own-markdown-emitter.md)
- [0041. Opt-in vault encryption with age](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0041-opt-in-vault-encryption-with-age.md)
- [0042. Empty note creation for machine authors](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0042-empty-note-creation-for-machine-authors.md)
- [0043. Write command for note content](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0043-write-command-for-note-content.md)
- [0044. Cross-document links between rendered notes](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0044-cross-document-links-between-rendered-notes.md)
- [0045. Vault render themes](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0045-vault-render-themes.md)
- [0046. Static site export with a `site` command and an `html` render format](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0046-static-site-export-with-a-site-command-and-an-html-render-format.md)
- [0047. Themes directory split by type](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0047-themes-directory-split-by-type.md)
- [0048. Site themes as stylesheets and assets](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0048-site-themes-as-stylesheets-and-assets.md)
- [0049. Shared Markdown walk with Typst and HTML emitters](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0049-shared-markdown-walk-with-typst-and-html-emitters.md)
- [0050. Page templates with minijinja](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0050-page-templates-with-minijinja.md)
- [0051. Browser-side code in TypeScript with a committed build](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0051-browser-side-code-in-typescript-with-committed-build.md)
- [0052. Client-side search as a TypeScript reimplementation of the query DSL](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0052-client-side-search-as-a-typescript-query-dsl.md)
- [0053. Syntax highlighting with Shiki in the browser](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0053-syntax-highlighting-with-shiki-in-the-browser.md)
- [0054. Site navigation and URL scheme](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0054-site-navigation-and-url-scheme.md)
- [0055. Theme directory layout with fonts, icons, and embedded assets](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0055-theme-directory-layout-with-fonts-icons-and-embedded-assets.md)
- [0056. Sidebar order, labels, landing notes, and a nav table](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0056-sidebar-order-labels-landing-notes-and-a-nav-table.md)
- [0057. The html artifact as a page with a files directory](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0057-html-artifact-as-a-page-with-a-files-directory.md)
- [0058. Theme templates overriding the built-in ones by name](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0058-theme-templates-overriding-the-built-in-ones-by-name.md)
