---
title: "Support Windows"
kind: feature
component: project
status: needs-discussion
origin: discussion
tags: [windows]
---
# Support Windows

Deferred during the v1 design (ADR 0020). v1 targets Unix only, that is
macOS and Linux.

## Open questions

- Materialized symlink views (ADR 0008) need symlink support, which on Windows
  requires Developer Mode or admin rights. Should Windows require that
  privilege, fall back to no materialized views, or use junctions or another
  mechanism? The answer sets the direction, which is why the todo needs
  discussion.
- How do path, case-folding and config-location differences apply on Windows?
- Windows has no `SIGPIPE`. `main()` resets it to `SIG_DFL` under
  `#[cfg(unix)]`, so `| head` and similar commands exit quietly instead of
  panicking. How should a Windows build handle a broken pipe? One option is to
  map a `BrokenPipe` error on stdout writes to a quiet exit instead of letting
  `println!` panic.
