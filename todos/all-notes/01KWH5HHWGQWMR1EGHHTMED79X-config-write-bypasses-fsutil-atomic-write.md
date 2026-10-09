# Global config write bypasses `fsutil::atomic_write` (and the fsutil invariant)

## Severity

Minor consistency/robustness issue.

## Problem

`src/fsutil.rs` module docs state the invariant: "Every write, symlink,
rename, and directory read or removal goes through here. No higher layer
calls `std::fs` ... directly."

`src/config/global.rs:55-67` (`write_at`) violates this: it calls
`std::fs::create_dir_all` and `std::fs::write` directly. Consequences:

1. The global config write is not atomic. A crash mid-write can leave a
   truncated `config.toml`, which then fails to parse on every subsequent
   run (`ConfigError::Parse`) until manually fixed. All note writes go
   through `atomic_write`; the config deserves the same.
2. The stated fsutil invariant is silently false, which misleads readers
   auditing filesystem behavior (this review found it by reading, exactly
   the failure the invariant is meant to prevent).

`fsutil` is `pub(crate)` (`src/lib.rs:20`), so `config` can use it directly.

## Suggested fix

Route `write_at` through `fsutil::create_dir_all` + `fsutil::atomic_write`
(mapping `FsError` into `ConfigError::Write`, or letting the error type
carry `FsError`). Then re-audit remaining direct `std::fs` uses in
non-fsutil modules and either route them through fsutil or narrow the
fsutil doc claim to what is actually true. Known direct uses at review
time: `src/config/global.rs:41/57/63`, `src/config/per_vault.rs:39`,
`src/scan.rs:135-136`, `src/vault/resolve.rs:143` (reads are arguably fine;
the doc only claims writes/renames/dir-ops, but `global.rs` writes).
