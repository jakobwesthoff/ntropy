# LSP dangling-link diagnostics (tier 2)

Deferred tier-2 ntropy language-server feature.

Publish `textDocument/publishDiagnostics` flagging links whose ULID resolves to
no note (ADR 0028 resolution). Computed on document open/change against the
in-memory session scan cache; reuses the link-extraction regex.

## Open questions

- Severity: warning vs. hint.
- Whether to also flag a stale slug whose ULID still resolves (cosmetic, fixed
  by `reconcile`) as a weaker diagnostic.
</content>
