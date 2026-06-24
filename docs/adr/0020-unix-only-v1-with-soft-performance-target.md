# 20. Unix-only v1 with soft performance target

Date: 2026-06-24

## Status

Accepted

## Context

Materialized views rely on symlinks (ADR 0008), which need elevated privilege
on Windows. The stateless scan (ADR 0002) re-reads notes on every query, so
the performance posture needs stating.

## Decision

v1 targets Unix (macOS and Linux) only. Windows is out of scope for v1; the
design is not contorted around Windows symlink restrictions.

Performance is a soft target: queries should feel instant for personal-scale
vaults (low thousands of notes), using a parallel directory walk. There is no
hard latency SLA and no benchmark suite to defend in v1; correctness is
preferred over micro-optimization.

## Consequences

- Symlink views work without special handling on the supported platforms.
- No Windows testing or symlink-fallback code in v1.
- No benchmark maintenance; if a concrete target is wanted later, it can be
  added without design changes.
