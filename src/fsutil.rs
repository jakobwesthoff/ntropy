// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Filesystem primitives: the single place in the crate that mutates the
//! filesystem.
//!
//! Every write, symlink, rename and directory wipe goes through here, which
//! localizes error handling and, crucially, the Unix-only symlink assumption
//! (ADR 0020). No higher layer calls `std::fs` mutators or
//! `std::os::unix::fs::symlink` directly; they call these helpers instead
//! (ADRs 0008, 0020).

use std::path::{Component, Path, PathBuf};

use ulid::Ulid;

/// An error from a filesystem primitive.
///
/// A single struct rather than a variant-per-operation: the human-readable
/// `action` plus the offending `path` already pin down what failed, and every
/// caller wants the same "while <action> `<path>`" shape.
#[derive(Debug, thiserror::Error)]
#[error("while {action} `{}`", path.display())]
pub struct FsError {
    action: &'static str,
    path: PathBuf,
    #[source]
    source: std::io::Error,
}

impl FsError {
    fn new(action: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self {
            action,
            path: path.into(),
            source,
        }
    }
}

type Result<T> = std::result::Result<T, FsError>;

// =============================================================================
// File reads
// =============================================================================

/// Read `path` to a string, returning `None` when it does not exist.
///
/// A missing file is a routine "nothing there yet" rather than an error, mirroring
/// how an absent config or `.gitignore` is treated as empty. Any other read
/// failure (permissions, I/O) surfaces as an [`FsError`].
pub fn read_to_string_if_exists(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(FsError::new("reading", path, e)),
    }
}

// =============================================================================
// File writes
// =============================================================================

/// Write `contents` to `path` atomically.
///
/// The bytes are first written to a uniquely named sibling temp file and then
/// renamed over the destination. `rename(2)` is atomic on a single filesystem,
/// so a reader either sees the old file or the fully written new one, never a
/// half-written note. The temp file is a sibling (same directory) so the rename
/// never crosses a filesystem boundary.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    // A ULID suffix keeps concurrent or repeated writes from clobbering each
    // other's temp file while still landing in the same directory.
    let tmp_name = match path.file_name() {
        Some(name) => format!("{}.{}.tmp", name.to_string_lossy(), Ulid::new()),
        None => format!("{}.tmp", Ulid::new()),
    };
    let tmp_path = parent.join(tmp_name);

    std::fs::write(&tmp_path, contents).map_err(|e| FsError::new("writing", &tmp_path, e))?;

    std::fs::rename(&tmp_path, path).map_err(|e| {
        // Best-effort cleanup so a failed rename does not leave the temp file
        // behind; the rename failure is the error we report.
        let _ = std::fs::remove_file(&tmp_path);
        FsError::new("renaming into place", path, e)
    })
}

/// Rename `from` to `to`.
pub fn rename(from: &Path, to: &Path) -> Result<()> {
    std::fs::rename(from, to).map_err(|e| FsError::new("renaming", from, e))
}

/// Remove `path` if it is a regular file or symlink.
pub fn remove_file(path: &Path) -> Result<()> {
    std::fs::remove_file(path).map_err(|e| FsError::new("removing file", path, e))
}

// =============================================================================
// Directories
// =============================================================================

/// Create `path` and all missing parents.
pub fn create_dir_all(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|e| FsError::new("creating directory", path, e))
}

/// Remove `path` only if it is an empty directory.
///
/// A non-empty directory and a missing directory are both left in place without
/// error. This lets a caller prune opportunistically — attempting every
/// directory it touched and relying on the no-op for those still holding entries
/// — rather than tracking emptiness itself.
pub fn remove_dir_if_empty(path: &Path) -> Result<()> {
    let mut entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(FsError::new("reading directory", path, e)),
    };
    if entries.next().is_some() {
        return Ok(());
    }
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(FsError::new("removing directory", path, e)),
    }
}

// =============================================================================
// Directory reads
// =============================================================================

/// List the immediate entries of `path` as `(path, file_type)` pairs.
///
/// A missing directory yields an empty list rather than an error, so a caller
/// can read a view tree that has not been materialized yet. The `file_type` is
/// the entry's own type and does not follow symlinks: a symlink reports as a
/// symlink, never as whatever it points at.
pub fn read_dir_entries(path: &Path) -> Result<Vec<(PathBuf, std::fs::FileType)>> {
    let iter = match std::fs::read_dir(path) {
        Ok(iter) => iter,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(FsError::new("reading directory", path, e)),
    };
    let mut entries = Vec::new();
    for entry in iter {
        let entry = entry.map_err(|e| FsError::new("reading directory entry", path, e))?;
        let file_type = entry
            .file_type()
            .map_err(|e| FsError::new("reading entry type", entry.path(), e))?;
        entries.push((entry.path(), file_type));
    }
    Ok(entries)
}

// =============================================================================
// Symlinks
// =============================================================================

/// Create a symlink at `link` whose stored target is `target`.
///
/// This is the sole site touching the symlink API, isolating the Unix-only
/// assumption (ADR 0020). `target` is stored verbatim, so pass a relative path
/// (see [`relative_path`]) to keep the vault relocatable (ADR 0008). Missing
/// parent directories of `link` are created first.
pub fn symlink(target: &Path, link: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        create_dir_all(parent)?;
    }
    std::os::unix::fs::symlink(target, link).map_err(|e| FsError::new("creating symlink", link, e))
}

/// Read the stored target of the symlink at `link`.
///
/// The target is returned exactly as stored — the relative path written by
/// [`symlink`] — without resolving it against the filesystem, so a dangling link
/// still yields its intended target. This is what lets a view sync compare an
/// existing leaf's target against the one it would write.
pub fn read_link(link: &Path) -> Result<PathBuf> {
    std::fs::read_link(link).map_err(|e| FsError::new("reading symlink", link, e))
}

// =============================================================================
// Pure path arithmetic
// =============================================================================

/// Compute the path of `target` relative to the directory `from_dir`.
///
/// Both inputs must be absolute and lexically normalized (no `.`/`..`
/// components), which holds for vault-internal paths. The result walks up out
/// of `from_dir` with `..` past the shared prefix and back down into `target`,
/// e.g. from `<vault>/by-tag/rust` to `<vault>/all-notes/x.md` yields
/// `../../all-notes/x.md`. This is pure path arithmetic and never touches the
/// filesystem, so it is unit-testable on its own.
pub fn relative_path(from_dir: &Path, target: &Path) -> PathBuf {
    let from: Vec<Component> = from_dir.components().collect();
    let to: Vec<Component> = target.components().collect();

    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for component in &to[common..] {
        result.push(component);
    }

    // Identical paths produce an empty buffer; `.` is the meaningful relative
    // form of "here".
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_walks_up_and_down_to_sibling_tree() {
        let from = Path::new("/vault/by-tag/rust");
        let target = Path::new("/vault/all-notes/x.md");
        assert_eq!(
            relative_path(from, target),
            PathBuf::from("../../all-notes/x.md")
        );
    }

    #[test]
    fn relative_path_handles_deep_nesting_depth() {
        let from = Path::new("/vault/by-tag/programming/rust");
        let target = Path::new("/vault/all-notes/x.md");
        assert_eq!(
            relative_path(from, target),
            PathBuf::from("../../../all-notes/x.md")
        );
    }

    #[test]
    fn relative_path_target_inside_from_dir() {
        let from = Path::new("/vault/a");
        let target = Path::new("/vault/a/b/c.md");
        assert_eq!(relative_path(from, target), PathBuf::from("b/c.md"));
    }

    #[test]
    fn relative_path_identical_is_dot() {
        let p = Path::new("/vault/a");
        assert_eq!(relative_path(p, p), PathBuf::from("."));
    }

    #[test]
    fn atomic_write_creates_file_with_contents_and_no_temp_leftovers() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.md");
        atomic_write(&path, b"hello").expect("atomic write");

        assert_eq!(std::fs::read(&path).expect("read back"), b"hello");

        // No `.tmp` siblings remain after a successful write.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "unexpected temp files: {leftovers:?}");
    }

    #[test]
    fn atomic_write_overwrites_existing_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.md");
        atomic_write(&path, b"first").expect("first write");
        atomic_write(&path, b"second").expect("second write");
        assert_eq!(std::fs::read(&path).expect("read back"), b"second");
    }

    #[test]
    fn remove_dir_if_empty_removes_an_empty_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let empty = dir.path().join("group");
        std::fs::create_dir(&empty).expect("mkdir");
        remove_dir_if_empty(&empty).expect("prune");
        assert!(!empty.exists());
    }

    #[test]
    fn remove_dir_if_empty_keeps_a_nonempty_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let group = dir.path().join("group");
        std::fs::create_dir(&group).expect("mkdir");
        std::fs::write(group.join("a.md"), b"x").expect("seed");
        remove_dir_if_empty(&group).expect("prune");
        assert!(group.is_dir(), "a populated directory must be left alone");
    }

    #[test]
    fn remove_dir_if_empty_ignores_a_missing_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("nope");
        remove_dir_if_empty(&missing).expect("missing is not an error");
    }

    #[test]
    fn read_dir_entries_of_a_missing_dir_is_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("nope");
        assert!(read_dir_entries(&missing).expect("read").is_empty());
    }

    #[test]
    fn read_dir_entries_reports_each_entry_type_without_following_symlinks() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir(root.join("sub")).expect("subdir");
        std::fs::write(root.join("file.md"), b"x").expect("file");
        // A symlink pointing at the real file must still report as a symlink.
        symlink(Path::new("file.md"), &root.join("link.md")).expect("symlink");

        let mut entries = read_dir_entries(root).expect("read");
        entries.sort_by_key(|(path, _)| path.clone());

        let by_name = |name: &str| {
            entries
                .iter()
                .find(|(path, _)| path.file_name().expect("name") == name)
                .map(|(_, ft)| *ft)
                .expect("entry present")
        };
        assert!(by_name("sub").is_dir());
        assert!(by_name("file.md").is_file());
        assert!(by_name("link.md").is_symlink());
    }

    #[test]
    fn read_link_returns_the_stored_target_even_when_dangling() {
        let dir = tempfile::tempdir().expect("temp dir");
        let link = dir.path().join("link.md");
        // Target does not exist: read_link still returns it verbatim.
        let target = Path::new("../all-notes/missing.md");
        symlink(target, &link).expect("symlink");
        assert_eq!(read_link(&link).expect("read_link"), target);
    }

    #[test]
    fn symlink_stores_relative_target_verbatim() {
        let dir = tempfile::tempdir().expect("temp dir");
        let canonical = dir.path().join("all-notes").join("x.md");
        std::fs::create_dir_all(canonical.parent().expect("parent")).expect("mkdir");
        std::fs::write(&canonical, b"body").expect("write note");

        let link = dir.path().join("by-tag").join("rust").join("link.md");
        let target = relative_path(link.parent().expect("link parent"), &canonical);
        symlink(&target, &link).expect("symlink");

        assert_eq!(std::fs::read_link(&link).expect("readlink"), target);
        // The relative link resolves back to the canonical body.
        assert_eq!(std::fs::read(&link).expect("read via link"), b"body");
    }

    #[test]
    fn read_to_string_if_exists_returns_contents() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("f.txt");
        std::fs::write(&path, b"hello").expect("seed");
        assert_eq!(
            read_to_string_if_exists(&path).expect("read"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn read_to_string_if_exists_returns_none_when_missing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("absent.txt");
        assert_eq!(read_to_string_if_exists(&path).expect("read"), None);
    }

    #[test]
    fn rename_moves_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("old.md");
        let to = dir.path().join("new.md");
        std::fs::write(&from, b"x").expect("seed");
        rename(&from, &to).expect("rename");
        assert!(!from.exists());
        assert!(to.exists());
    }

    #[test]
    fn fserror_reports_action_and_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("nope").join("deep.md");
        // Renaming a non-existent source surfaces a contextual error.
        let err = rename(&missing, &dir.path().join("dst.md")).expect_err("should fail");
        let msg = err.to_string();
        assert!(msg.contains("renaming"), "message was: {msg}");
        assert!(msg.contains("deep.md"), "message was: {msg}");
    }
}
