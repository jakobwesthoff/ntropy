// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Resolving which vault an invocation operates on (ADRs 0016, 0026).
//!
//! The order is `--vault` > `$NTROPY_VAULT` > cwd walk-up > global default. The
//! walk-up honors both signals at each ancestor directory: a `.ntropy-vault`
//! pointer file and a `.ntropy/` directory, with the nearest ancestor winning
//! and the pointer winning over a `.ntropy/` in the same directory (it is an
//! explicit redirect). A pointer that does not lead to a real vault is a hard
//! error rather than a silent fall-through, so misconfiguration is visible.

use std::path::{Path, PathBuf};

use super::layout::{self, POINTER_FILE};

/// Inputs to vault resolution, gathered by the binary from flags, environment
/// and config so this function stays pure and testable.
#[derive(Debug, Default)]
pub struct ResolveOptions {
    /// The `--vault` flag value.
    pub explicit: Option<PathBuf>,
    /// The `$NTROPY_VAULT` environment value.
    pub env: Option<PathBuf>,
    /// The directory to begin the walk-up from (normally the cwd).
    pub start_dir: Option<PathBuf>,
    /// The global config's `default_vault`.
    pub global_default: Option<PathBuf>,
}

/// Which rule resolved the active vault, for reporting (`info`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveSource {
    /// The `--vault` flag.
    Explicit,
    /// The `$NTROPY_VAULT` environment variable.
    Env,
    /// A `.ntropy-vault` pointer file found during walk-up (its path).
    Pointer(PathBuf),
    /// A `.ntropy/` directory found by walking up from the start directory.
    WalkUp,
    /// The global config's default vault.
    GlobalDefault,
}

/// Why a vault could not be resolved.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("`{}` is not a vault (no .ntropy directory)", .0.display())]
    NotAVault(PathBuf),
    #[error("the `.ntropy-vault` pointer at `{}` is broken: {reason}", pointer.display())]
    BrokenPointer { pointer: PathBuf, reason: String },
    #[error(
        "no vault found; pass --vault, set $NTROPY_VAULT, run inside a vault, or set a default"
    )]
    NoVault,
}

/// Resolve the vault root directory from the given options.
pub fn resolve(opts: &ResolveOptions) -> Result<PathBuf, ResolveError> {
    resolve_with_source(opts).map(|(root, _)| root)
}

/// Resolve the vault root, also reporting which rule matched (for `info`).
pub fn resolve_with_source(
    opts: &ResolveOptions,
) -> Result<(PathBuf, ResolveSource), ResolveError> {
    // 1. Explicit `--vault` wins outright; it must already be a vault.
    if let Some(path) = &opts.explicit {
        return Ok((require_vault(path)?, ResolveSource::Explicit));
    }

    // 2. Then `$NTROPY_VAULT`.
    if let Some(path) = &opts.env {
        return Ok((require_vault(path)?, ResolveSource::Env));
    }

    // 3. Then the cwd walk-up.
    if let Some(start) = &opts.start_dir
        && let Some(found) = walk_up(start)?
    {
        return Ok(found);
    }

    // 4. Finally the global default.
    if let Some(path) = &opts.global_default {
        return Ok((require_vault(path)?, ResolveSource::GlobalDefault));
    }

    Err(ResolveError::NoVault)
}

/// Validate that `path` is a vault and canonicalize it.
fn require_vault(path: &Path) -> Result<PathBuf, ResolveError> {
    if !layout::is_vault(path) {
        return Err(ResolveError::NotAVault(path.to_path_buf()));
    }
    canonicalize(path).ok_or_else(|| ResolveError::NotAVault(path.to_path_buf()))
}

/// Walk from `start` up to the filesystem root, returning the first vault found
/// via either signal. The pointer is checked first so it wins in a tie.
fn walk_up(start: &Path) -> Result<Option<(PathBuf, ResolveSource)>, ResolveError> {
    for dir in start.ancestors() {
        let pointer = dir.join(POINTER_FILE);
        if pointer.is_file() {
            let target = resolve_pointer(&pointer)?;
            return Ok(Some((target, ResolveSource::Pointer(pointer))));
        }
        if layout::is_vault(dir) {
            return Ok(canonicalize(dir).map(|root| (root, ResolveSource::WalkUp)));
        }
    }
    Ok(None)
}

/// Resolve a `.ntropy-vault` pointer file to its target vault.
///
/// The single line is a path relative to the pointer's own directory, or
/// absolute, or `~`-prefixed. The target must be an existing vault.
fn resolve_pointer(pointer: &Path) -> Result<PathBuf, ResolveError> {
    let broken = |reason: &str| ResolveError::BrokenPointer {
        pointer: pointer.to_path_buf(),
        reason: reason.to_string(),
    };

    let content = std::fs::read_to_string(pointer).map_err(|e| broken(&e.to_string()))?;
    let line = content.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return Err(broken("the pointer file is empty"));
    }

    let raw = expand_tilde(line);
    let target = if raw.is_absolute() {
        raw
    } else {
        // Relative to the marker's own directory.
        pointer.parent().unwrap_or_else(|| Path::new(".")).join(raw)
    };

    if !layout::is_vault(&target) {
        return Err(broken(&format!("`{}` is not a vault", target.display())));
    }
    canonicalize(&target).ok_or_else(|| broken("the target could not be resolved"))
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    } else if path == "~"
        && let Some(home) = home_dir()
    {
        return home;
    }
    PathBuf::from(path)
}

/// The user's home directory, if known.
fn home_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

/// Canonicalize a path, dropping the error (callers turn `None` into the
/// appropriate domain error).
fn canonicalize(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Make `path` look like a vault by creating its `.ntropy/` dir.
    fn make_vault(path: &Path) {
        fs::create_dir_all(path.join(".ntropy")).expect("mkdir .ntropy");
    }

    fn canonical(path: &Path) -> PathBuf {
        fs::canonicalize(path).expect("canonicalize")
    }

    #[test]
    fn explicit_vault_wins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path().join("v");
        make_vault(&vault);
        let resolved = resolve(&ResolveOptions {
            explicit: Some(vault.clone()),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&vault));
    }

    #[test]
    fn explicit_non_vault_errors() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = resolve(&ResolveOptions {
            explicit: Some(dir.path().to_path_buf()),
            ..Default::default()
        })
        .expect_err("not a vault");
        assert!(matches!(err, ResolveError::NotAVault(_)));
    }

    #[test]
    fn env_used_when_no_explicit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path().join("v");
        make_vault(&vault);
        let resolved = resolve(&ResolveOptions {
            env: Some(vault.clone()),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&vault));
    }

    #[test]
    fn explicit_precedes_env() {
        let dir = tempfile::tempdir().expect("temp dir");
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        make_vault(&a);
        make_vault(&b);
        let resolved = resolve(&ResolveOptions {
            explicit: Some(a.clone()),
            env: Some(b),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&a));
    }

    #[test]
    fn walk_up_finds_ntropy_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path().join("vault");
        make_vault(&vault);
        let nested = vault.join("sub").join("deep");
        fs::create_dir_all(&nested).expect("nested");

        let resolved = resolve(&ResolveOptions {
            start_dir: Some(nested),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&vault));
    }

    #[test]
    fn pointer_redirects_to_external_vault() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("project");
        let vault = dir.path().join("notes");
        make_vault(&vault);
        fs::create_dir_all(&project).expect("project");
        // Relative pointer from the project to a sibling vault.
        fs::write(project.join(".ntropy-vault"), "../notes\n").expect("pointer");

        let resolved = resolve(&ResolveOptions {
            start_dir: Some(project),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&vault));
    }

    #[test]
    fn pointer_wins_over_ntropy_in_same_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let here = dir.path().join("here");
        let other = dir.path().join("other");
        make_vault(&here); // `here` is itself a vault...
        make_vault(&other);
        // ...but a pointer in the same dir redirects elsewhere.
        fs::write(here.join(".ntropy-vault"), "../other\n").expect("pointer");

        let resolved = resolve(&ResolveOptions {
            start_dir: Some(here),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&other));
    }

    #[test]
    fn nearest_signal_wins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outer = dir.path().join("outer");
        let inner = outer.join("inner");
        make_vault(&outer);
        make_vault(&inner);
        let start = inner.join("sub");
        fs::create_dir_all(&start).expect("start");

        let resolved = resolve(&ResolveOptions {
            start_dir: Some(start),
            ..Default::default()
        })
        .expect("resolve");
        // The inner vault is nearer the start directory.
        assert_eq!(resolved, canonical(&inner));
    }

    #[test]
    fn broken_pointer_is_hard_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("project");
        fs::create_dir_all(&project).expect("project");
        fs::write(project.join(".ntropy-vault"), "./does-not-exist\n").expect("pointer");

        let err = resolve(&ResolveOptions {
            start_dir: Some(project),
            // A global default exists, but a broken pointer must not fall back.
            global_default: None,
            ..Default::default()
        })
        .expect_err("broken pointer");
        assert!(matches!(err, ResolveError::BrokenPointer { .. }));
    }

    #[test]
    fn absolute_pointer_is_honored() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("project");
        let vault = dir.path().join("vault");
        make_vault(&vault);
        fs::create_dir_all(&project).expect("project");
        fs::write(
            project.join(".ntropy-vault"),
            format!("{}\n", vault.display()),
        )
        .expect("pointer");

        let resolved = resolve(&ResolveOptions {
            start_dir: Some(project),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&vault));
    }

    #[test]
    fn falls_back_to_global_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path().join("default-vault");
        make_vault(&vault);
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).expect("elsewhere");

        let resolved = resolve(&ResolveOptions {
            start_dir: Some(elsewhere),
            global_default: Some(vault.clone()),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(resolved, canonical(&vault));
    }

    #[test]
    fn reports_the_resolution_source() {
        let dir = tempfile::tempdir().expect("temp dir");
        let explicit = dir.path().join("explicit");
        let env = dir.path().join("env");
        let walk = dir.path().join("walk");
        let dflt = dir.path().join("default");
        for v in [&explicit, &env, &walk, &dflt] {
            make_vault(v);
        }

        let source = |opts: &ResolveOptions| resolve_with_source(opts).expect("resolve").1;

        assert_eq!(
            source(&ResolveOptions {
                explicit: Some(explicit.clone()),
                ..Default::default()
            }),
            ResolveSource::Explicit
        );
        assert_eq!(
            source(&ResolveOptions {
                env: Some(env.clone()),
                ..Default::default()
            }),
            ResolveSource::Env
        );
        assert_eq!(
            source(&ResolveOptions {
                start_dir: Some(walk.clone()),
                ..Default::default()
            }),
            ResolveSource::WalkUp
        );
        assert_eq!(
            source(&ResolveOptions {
                start_dir: Some(dir.path().join("nowhere")),
                global_default: Some(dflt.clone()),
                ..Default::default()
            }),
            ResolveSource::GlobalDefault
        );
    }

    #[test]
    fn reports_pointer_as_the_source() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("project");
        let vault = dir.path().join("notes");
        make_vault(&vault);
        fs::create_dir_all(&project).expect("project");
        fs::write(project.join(".ntropy-vault"), "../notes\n").expect("pointer");

        let (_root, source) = resolve_with_source(&ResolveOptions {
            start_dir: Some(project.clone()),
            ..Default::default()
        })
        .expect("resolve");
        assert_eq!(
            source,
            ResolveSource::Pointer(project.join(".ntropy-vault"))
        );
    }

    #[test]
    fn no_vault_anywhere_is_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let elsewhere = dir.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).expect("elsewhere");
        let err = resolve(&ResolveOptions {
            start_dir: Some(elsewhere),
            ..Default::default()
        })
        .expect_err("no vault");
        assert!(matches!(err, ResolveError::NoVault));
    }
}
