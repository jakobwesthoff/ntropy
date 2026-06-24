# Windows support (post-v1)

Deferred from v1 per ADR 0020. v1 targets Unix (macOS + Linux) only.

## To decide later

- Materialized symlink views (ADR 0008) depend on symlink support, which needs
  Developer Mode / admin on Windows. Options: require the privilege, fall back
  to no materialized views, or use junctions/another mechanism.
- Path, case-folding, and config-location differences.
