// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Global config: the default vault (ADR 0016).
//!
//! Located in the OS-native config directory via the `directories` crate
//! (`~/.config/ntropy/config.toml` on Linux, `~/Library/Application
//! Support/ntropy/config.toml` on macOS). The only v1 field is `default_vault`,
//! used when no `--vault`, `$NTROPY_VAULT` or cwd walk-up resolves a vault.
//!
//! The path-resolving helpers and the IO helpers are kept separate: `*_at`
//! functions take an explicit path so they are testable against a temp file,
//! while the no-suffix functions resolve the real OS location.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use super::ConfigError;
use crate::fsutil;

/// The config file name within the application config directory.
const CONFIG_FILE: &str = "config.toml";

/// The parsed global configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// The fallback vault path used when nothing else resolves one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_vault: Option<PathBuf>,
}

/// The OS-native path to the global config file, if a config dir is known.
pub fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "ntropy").map(|dirs| dirs.config_dir().join(CONFIG_FILE))
}

/// Load the global config from `path`, treating a missing file as default.
pub fn load_at(path: &Path) -> Result<GlobalConfig, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(GlobalConfig::default()),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Write the global config to `path`, creating parent directories as needed.
///
/// The file is replaced atomically, so a crash mid-write leaves the old config
/// whole rather than a truncated one that fails to parse on every later run.
/// The replacement is a new file: it gets the default permissions of one, not
/// those of the file it replaces.
///
/// A `path` that is a symlink, as when the config lives in a dotfiles
/// repository, stays one: the write lands in the file the link points at,
/// which is created if the link dangles.
pub fn write_at(path: &Path, config: &GlobalConfig) -> Result<(), ConfigError> {
    let write_error = |source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    };
    let target = follow_symlinks(path).map_err(write_error)?;
    // Only `/` or an empty path lacks a parent, and neither can hold the
    // file; the write below reports that.
    let parent = target.parent().unwrap_or(Path::new(""));
    fsutil::create_dir_all(parent).map_err(|e| write_error(e.into_io()))?;
    let text = toml::to_string_pretty(config)?;
    fsutil::atomic_write(&target, text.as_bytes()).map_err(|e| write_error(e.into_io()))
}

/// The most symlinks [`follow_symlinks`] follows before giving up, the same
/// bound Linux puts on path resolution.
const MAX_SYMLINK_HOPS: usize = 40;

/// The path a write to `path` should replace: `path` itself, or the end of
/// the chain of symlinks starting there, whether or not that end exists.
///
/// Anything that is not a symlink, including a path that cannot be looked at,
/// ends the chain; writing there then reports the real problem.
fn follow_symlinks(path: &Path) -> std::io::Result<PathBuf> {
    let mut current = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_HOPS {
        let is_symlink = std::fs::symlink_metadata(&current)
            .is_ok_and(|metadata| metadata.file_type().is_symlink());
        if !is_symlink {
            return Ok(current);
        }
        // A relative target is relative to the link's own directory; joining
        // an absolute one replaces the base entirely. A symlink always has a
        // name, so it always has a parent, if only the empty one.
        let target = std::fs::read_link(&current)?;
        current = current.parent().unwrap_or(Path::new("")).join(target);
    }
    Err(std::io::Error::other(format!(
        "more than {MAX_SYMLINK_HOPS} symbolic links to follow"
    )))
}

/// Load the global config from its OS-native location.
///
/// Returns the default config when no config directory is known or the file is
/// absent, so callers always get a usable value.
pub fn load() -> Result<GlobalConfig, ConfigError> {
    match config_path() {
        Some(path) => load_at(&path),
        None => Ok(GlobalConfig::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_is_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cfg = load_at(&dir.path().join("config.toml")).expect("load");
        assert_eq!(cfg, GlobalConfig::default());
        assert!(cfg.default_vault.is_none());
    }

    #[test]
    fn write_then_load_roundtrips() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A nested path exercises parent-directory creation.
        let path = dir.path().join("ntropy").join("config.toml");
        let cfg = GlobalConfig {
            default_vault: Some(PathBuf::from("/Users/jakob/notes")),
        };
        write_at(&path, &cfg).expect("write");
        assert_eq!(load_at(&path).expect("load"), cfg);
    }

    fn sample() -> GlobalConfig {
        GlobalConfig {
            default_vault: Some(PathBuf::from("/Users/jakob/notes")),
        }
    }

    #[test]
    fn write_at_replaces_the_file_rather_than_rewriting_it() {
        // A new inode is the visible trace of writing a temporary file and
        // renaming it over the old one: a crash mid-write leaves the old file
        // whole instead of a truncated one that fails to parse.
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("config.toml");
        write_at(&path, &GlobalConfig::default()).expect("first write");
        let before = std::fs::metadata(&path).expect("stat").ino();

        write_at(&path, &sample()).expect("second write");
        let after = std::fs::metadata(&path).expect("stat").ino();

        assert_ne!(before, after, "the file was rewritten in place");
        assert_eq!(load_at(&path).expect("load"), sample());
    }

    #[test]
    fn write_at_leaves_no_temp_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_at(&dir.path().join("config.toml"), &sample()).expect("write");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("config.toml")]);
    }

    #[test]
    fn write_at_keeps_a_symlinked_config_a_symlink() {
        // A config kept in a dotfiles repository and linked into place must
        // stay linked; the write lands in the file the link points at.
        let dir = tempfile::tempdir().expect("temp dir");
        let dotfiles = dir.path().join("dotfiles");
        std::fs::create_dir(&dotfiles).expect("mkdir");
        let target = dotfiles.join("ntropy.toml");
        std::fs::write(&target, "").expect("seed target");
        let link = dir.path().join("config.toml");
        std::os::unix::fs::symlink("dotfiles/ntropy.toml", &link).expect("symlink");

        write_at(&link, &sample()).expect("write");

        assert!(link.symlink_metadata().expect("lstat").is_symlink());
        assert_eq!(load_at(&target).expect("load target"), sample());
    }

    #[test]
    fn write_at_writes_through_a_dangling_symlink() {
        // A link set up before its target exists, into a directory that does
        // not exist yet either.
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("dotfiles").join("ntropy.toml");
        let link = dir.path().join("config.toml");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        write_at(&link, &sample()).expect("write");

        assert!(link.symlink_metadata().expect("lstat").is_symlink());
        assert_eq!(load_at(&target).expect("load target"), sample());
    }

    #[test]
    fn write_at_rejects_a_symlink_cycle() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("config.toml");
        let second = dir.path().join("other.toml");
        std::os::unix::fs::symlink(&second, &first).expect("symlink");
        std::os::unix::fs::symlink(&first, &second).expect("symlink");

        let err = write_at(&first, &sample()).expect_err("a cycle has no target");
        assert!(matches!(err, ConfigError::Write { ref path, .. } if *path == first));
    }

    #[test]
    fn write_at_reports_a_failed_directory_creation_as_a_config_write() {
        // The parent is a regular file, so no directory can be made there.
        let dir = tempfile::tempdir().expect("temp dir");
        let blocker = dir.path().join("ntropy");
        std::fs::write(&blocker, "").expect("seed file");
        let path = blocker.join("config.toml");

        let err = write_at(&path, &sample()).expect_err("parent is a file");
        assert!(matches!(err, ConfigError::Write { path: ref reported, .. } if *reported == path));
    }

    #[test]
    fn write_at_reports_a_failed_replacement_as_a_config_write() {
        // A directory where the file belongs cannot be renamed over.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("config.toml");
        std::fs::create_dir(&path).expect("mkdir");

        let err = write_at(&path, &sample()).expect_err("path is a directory");
        assert!(matches!(err, ConfigError::Write { path: ref reported, .. } if *reported == path));
    }

    #[test]
    fn default_serializes_without_vault_key() {
        let text = toml::to_string_pretty(&GlobalConfig::default()).expect("serialize");
        assert!(!text.contains("default_vault"), "got: {text:?}");
    }

    #[test]
    fn config_path_is_under_ntropy_dir() {
        // The exact root is OS-dependent, but the file always sits under an
        // `ntropy` directory and is named `config.toml`.
        if let Some(path) = config_path() {
            assert!(path.ends_with("ntropy/config.toml"));
        }
    }
}
