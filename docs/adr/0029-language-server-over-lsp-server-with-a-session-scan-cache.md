# 29. Language server over lsp-server with a session scan cache

Date: 2026-06-25

## Status

Accepted

## Context

ntropy cannot inject into an external editor buffer, so ergonomic link and tag
authoring needs an editor-side integration. A Language Server Protocol server is
the editor-agnostic way to provide completion and navigation. ntropy is
otherwise synchronous (parallelism via `ignore` threads, no async runtime) and
dependency-conscious (ADR 0024).

## Decision

Ship a language server, served over stdio by an `ntropy lsp` subcommand.

- **Stack:** `lsp-server` + `lsp-types`, synchronous. `lsp-server` is the
  rust-analyzer transport: a blocking JSON-RPC loop over channels, no async
  runtime, matching ntropy's synchronous design. This stack is adopted for v1 and
  confirmed by building the server; `tower-lsp-server` is the identified fallback
  if the synchronous model proves limiting.
- **v1 scope:** completion (links and frontmatter tags),
  `textDocument/definition`, `textDocument/documentLink`, and `workspace/symbol`.
  Backlinks, hover, diagnostics and document symbols are deferred (tier-2 todos).
- **Data model:** a per-process, in-memory scan cache of
  `(ULID, title, tags, path)`, keyed by resolved vault root and populated
  lazily. It refreshes by a full vault rescan on
  `workspace/didChangeWatchedFiles` (the server registers a watch on `**/*.md`),
  with a rescan on `didOpen` as the fallback when a client's watched-file support
  is weak. No incremental patching and no open-buffer overlay: the cache reflects
  saved on-disk state.

The cache is process-local and ephemeral, rebuilt from the filesystem each
session, so it is a session cache rather than a persisted derived index and does
not contradict ADR 0002.

## Rejected alternatives

- **`tower-lsp` / async stack:** pulls `tokio` into a synchronous project for a
  completion workload that gains nothing from async. (`tower-lsp` itself is
  unmaintained; `tower-lsp-server` is the live fork, kept only as the fallback
  above.)
- **`[[` as the link-completion trigger:** a non-standard bracket pair used only
  to trigger; plain `[` is unobtrusive (the list closes or empties as unrelated
  text is typed) and needs no rewrite of a second bracket.
- **Incremental cache patching:** a full rescan is always correct at
  personal-vault scale (ADR 0020); per-entry patching adds bug surface for no
  needed gain.
- **Open-buffer overlay:** would only affect the open note's own metadata before
  save; not worth merging buffer state into the cache.
- **Markdown AST or tree-sitter parser for links:** the regex extraction of
  ADR 0028 suffices; a parser would contradict ntropy shipping no Markdown
  parser.
- **Spanned YAML parser for tag context:** the two frontmatter forms are detected
  by a local cursor scan; a YAML parser in the per-keystroke path is overkill.

## Consequences

- One server process per editor workspace handles all open notes; a second
  editor window is a separate process, so sessions cache independently.
- Per-keystroke completion runs against memory, not the filesystem.
- Edits to the open note's own title or tags are not reflected in candidate
  lists until the file is saved and the watch fires.
- A full rescan per watched-file event is O(vault); acceptable at the personal
  scale of ADR 0020, consistent with the full-rebuild choice for views.
</content>
