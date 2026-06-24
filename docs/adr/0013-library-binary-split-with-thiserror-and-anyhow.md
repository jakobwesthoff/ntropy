# 13. Library/binary split with thiserror and anyhow

Date: 2026-06-24

## Status

Accepted

## Context

ntropy needs a clear boundary between domain logic and the CLI, and a
consistent error-handling strategy on each side.

## Decision

Split the crate into a library and a binary:

- The library implements all domain functionality (scanning, parsing, query
  evaluation, view generation, reconcile, etc.).
- The binary (`main.rs`) is the CLI interface over the library.

Errors:

- The library uses `thiserror`, with errors grouped into semantically
  meaningful types rather than one catch-all enum.
- The binary uses `anyhow` for top-level error handling and reporting.

## Consequences

- Domain logic is reusable and testable independently of the CLI.
- Callers of the library match on specific, semantic error variants; the CLI
  collapses them to `anyhow` with context for human-facing messages.
- The library must define and maintain deliberate error groupings, which is
  more upfront design than a single error type.
