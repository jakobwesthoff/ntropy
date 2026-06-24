# 21. Testing strategy with insta across all layers

Date: 2026-06-24

## Status

Accepted

## Context

The library/binary split (ADR 0013) makes domain logic unit-testable. Much of
the risk is in filesystem-heavy view generation and reconcile, and in the CLI
contract (exit codes, stdout/stderr, `--strict`). The interactive picker is a
TUI and impractical to test directly.

## Decision

Three layers, with `insta` as the assertion tool throughout:

- Unit tests of domain logic (DSL parser, query evaluation, slug/tag
  normalization, ULID/filename parsing, view path computation), snapshotted
  with `insta` where it fits.
- Integration tests over temporary vaults (e.g. `tempfile`/`assert_fs`):
  exercise create, reconcile, and view generation end-to-end and snapshot the
  resulting view trees and output with `insta`.
- CLI contract tests with `insta-cmd`: run the real binary and snapshot
  stdout, stderr, and exit code.

Selection logic is kept separable from picker rendering so it is unit-testable;
the TUI itself is not tested automatically.

## Consequences

- One assertion style (`insta`) spans unit, integration, and CLI tests.
- Filesystem-heavy view/reconcile logic is covered by real temp-vault runs.
- The CLI contract is verified against the actual binary.
- Picker UI behavior is validated manually; only its underlying logic is
  unit-tested.
