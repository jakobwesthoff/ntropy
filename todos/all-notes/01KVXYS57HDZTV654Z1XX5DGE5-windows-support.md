# Windows support (post-v1)

Deferred from v1 per ADR 0020. v1 targets Unix (macOS + Linux) only.

## To decide later

- Materialized symlink views (ADR 0008) depend on symlink support, which needs
  Developer Mode / admin on Windows. Options: require the privilege, fall back
  to no materialized views, or use junctions/another mechanism.
- Path, case-folding, and config-location differences.
- Windows has no `SIGPIPE`: `main()`'s `#[cfg(unix)]` reset of `SIGPIPE` to
  `SIG_DFL` (so `| head` and similar exit quietly instead of panicking) has no
  Windows equivalent. A Windows build needs its stdout writes to map a
  `BrokenPipe` error to a quiet exit instead of letting `println!` panic.
