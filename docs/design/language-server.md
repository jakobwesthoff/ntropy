# Language server

How ntropy integrates with editors to author and navigate links and tags.
Consolidates
[ADR 0029](../adr/0029-language-server-over-lsp-server-with-a-session-scan-cache.md),
[ADR 0028](../adr/0028-note-to-note-links-as-standard-markdown-links.md),
[ADR 0004](../adr/0004-note-identity-and-filename-strategy.md),
[ADR 0002](../adr/0002-stateless-filesystem-scanning-over-a-derived-index.md), and
[ADR 0023](../adr/0023-slug-tag-and-disambiguator-normalization-rules.md).

## Why a language server

ntropy cannot write into an external editor's buffer, so link and tag authoring
inside the editor has to come from an editor-side integration. An LSP server is
editor-agnostic: one implementation serves every LSP-capable editor.

## Process and lifecycle

The server runs over stdio, started by `ntropy lsp`. An editor launches one
server process per workspace and multiplexes every open note through it over a
single connection, each document identified by URI. A second editor window is a
separate process. The server resolves the vault for a document with the same
resolution ntropy's CLI uses (walk-up, pointer file) and keys its state by vault
root, so even one editor opening several vaults stays correct.

Handshake and sync use `initialize`/`initialized`/`shutdown`/`exit` over
`lsp-server`'s connection helper. Documents sync as `TextDocumentSyncKind::FULL`
(the whole buffer on each change). Buffer text is read only to determine cursor
context for a request; it is not merged into the scan cache.

## The session scan cache

Every feature reads candidates from one in-memory cache per vault root: a list
of `(ULID, title, tags, path)`, built by the same scan the CLI uses (ADR 0002).

- Populated lazily on first need.
- Refreshed by a full vault rescan on `workspace/didChangeWatchedFiles`; the
  server registers a watch on `**/*.md`, so editor file events (create, change,
  delete, including notes never opened) drive the refresh.
- Fallback: a rescan on `didOpen` where a client's watched-file support is weak.
- No incremental patching and no open-buffer overlay. The cache reflects saved
  on-disk state, so an unsaved title or tag change in the open note becomes
  visible to other features only after the file is saved.

Per-keystroke completion therefore never touches the filesystem. The cache is
ephemeral and process-local: a session cache, not a persisted index (ADR 0002).

## Completion

Completion returns `CompletionList { is_incomplete: true }`, so the editor
re-queries on each keystroke and the server re-evaluates context and re-ranks.
Context is detected locally from the buffer at the cursor; the server never
parses the whole document.

### Links

Trigger character `[`. After `[`, the text up to the cursor is a fuzzy query,
matched with `nucleo` (ADR 0027) against each note's **title and tags**. On
accept, the item's `textEdit` spans from just after `[` to the cursor and
inserts the target note's canonical title and its standard-Markdown target
(ADR 0028):

    [Quarterly Review](01jzq3w8xk7m2n4p0qr5tt9abc-quarterly-review.md)

`InsertTextFormat::Snippet` places the final cursor after the link. The typed
query is replaced by the canonical title; a custom display text is an edit the
author makes afterward.

### Tags

Completion offers the vault's existing normalized tag set (ADR 0023), fuzzy
filtered by the partial and slash-hierarchy aware (typing `programming/` narrows
to children). The completion point is found without a YAML parser, by locating
the cursor inside the frontmatter `tags` value in either authored form:

1. Confirm the cursor is within frontmatter: line 0 is `---` and the cursor is
   above the closing `---`.
2. Flow form `tags: [a, b, c|]`: the cursor line up to the cursor matches
   `tags:` followed by `[` with no closing `]`; the partial is the text after the
   last `[` or `,`.
3. Block form (a `tags:` line followed by `- a` / `- b|` items): the cursor line
   matches `^\s*-\s*`, and walking up over contiguous list-item lines reaches a
   `tags:` key. The partial is the text after the `-`.

Multi-line flow arrays are out of scope for v1 (single-line flow only). An
unsupported layout simply yields no completion and the author types the tag by
hand.

## Navigation

- **`textDocument/definition`**: on a link, resolve the target by ULID glob
  (ADR 0028) and jump to the note.
- **`textDocument/documentLink`**: hand the editor each link's resolved target
  URI, so links are click-to-open and rendered as links independent of
  `definition`. Both reuse the link-extraction regex of ADR 0028.
- **`workspace/symbol`**: every note as a symbol named by its title and located
  at its file. With `nucleo` ranking the query, this is a command-palette jump to
  any note by title across the vault.

## Open points

- Whether the synchronous `lsp-server` stack suffices is settled by building the
  v1 server; `tower-lsp-server` is the identified fallback (ADR 0029).
- Tier-2 features (backlinks/references, hover preview, dangling-link
  diagnostics, document-symbol outline) are deferred; see the todos referenced
  from `todos/01kvxwqq5vbjekr578jffved5m-linking-and-language-server.md`.
</content>
