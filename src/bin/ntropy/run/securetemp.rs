// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Somewhere outside the vault to put plaintext (ADR 0041).
//!
//! Editing an encrypted note means handing an editor a real file, and that file
//! must not be the ciphertext. It goes to `$XDG_RUNTIME_DIR` where one exists —
//! tmpfs on Linux, so the plaintext never reaches a disk — and otherwise to the
//! system temp directory, which is the best available.
//!
//! What matters either way is that it is *outside the vault*: a sync provider
//! watches the vault directory, so a temp file inside it would be uploaded on
//! whatever platform the user happens to be on. On macOS there is no tmpfs
//! equivalent and the gap between "temp file on disk" and "plaintext never on
//! disk" is covered by FileVault.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Where plaintext may be staged.
pub fn runtime_dir() -> PathBuf {
    runtime_dir_from(
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        &std::env::temp_dir(),
    )
}

/// The injectable core of [`runtime_dir`].
///
/// `XDG_RUNTIME_DIR` is honoured only when it names a directory that exists: a
/// stale or empty value in the environment must fall back rather than send the
/// plaintext somewhere unwritable.
pub fn runtime_dir_from(xdg: Option<&OsStr>, fallback: &Path) -> PathBuf {
    match xdg {
        Some(value) if !value.is_empty() && Path::new(value).is_dir() => PathBuf::from(value),
        _ => fallback.to_path_buf(),
    }
}

/// A plaintext file living outside the vault for as long as it is held.
///
/// Created `0600`, overwritten and removed on drop. The overwrite is a
/// best-effort measure and not a guarantee — a copying filesystem or a
/// wear-levelling SSD may keep the old blocks — but it removes the obvious
/// copy at no cost.
pub struct SecureTempFile {
    dir: tempfile::TempDir,
    path: PathBuf,
}

impl SecureTempFile {
    /// Create an empty `0600` file named `name` in a private directory.
    ///
    /// The name is preserved rather than randomized because editors decide
    /// syntax highlighting, and much else, from the extension. A private
    /// directory per file is what makes that possible without two concurrent
    /// edits of the same note colliding.
    pub fn create(name: &str) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::Builder::new()
            .prefix("ntropy-")
            .tempdir_in(runtime_dir())
            .context("while creating a private directory for the decrypted note")?;
        let path = dir.path().join(name);

        std::fs::write(&path, b"")
            .with_context(|| format!("while creating `{}`", path.display()))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("while restricting `{}`", path.display()))?;

        Ok(Self { dir, path })
    }

    /// The file's path, for handing to an editor.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the file back.
    pub fn read(&self) -> Result<String> {
        std::fs::read_to_string(&self.path)
            .with_context(|| format!("while reading `{}`", self.path.display()))
    }

    /// Replace the file's contents.
    pub fn write(&self, content: &str) -> Result<()> {
        std::fs::write(&self.path, content)
            .with_context(|| format!("while writing `{}`", self.path.display()))
    }

    /// Give up ownership so the file survives this scope.
    ///
    /// Used where an edit could not be written back and the user's work would
    /// otherwise be destroyed by the cleanup. The path is returned so it can be
    /// named in the error that follows.
    pub fn keep(self) -> PathBuf {
        let path = self.path.clone();
        // Leaking the handle is the point: dropping it would remove the
        // directory holding the work this is trying to preserve.
        std::mem::forget(self);
        path
    }
}

impl Drop for SecureTempFile {
    fn drop(&mut self) {
        // Overwrite before unlinking so the plaintext does not simply become
        // unreferenced-but-present.
        if let Ok(length) = std::fs::metadata(&self.path).map(|m| m.len()) {
            let _ = std::fs::write(&self.path, vec![0u8; length as usize]);
        }
        let _ = std::fs::remove_file(&self.path);
        // `dir` removes itself; naming it here keeps the field read.
        let _ = &self.dir;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn xdg_runtime_dir_is_preferred_when_it_exists() {
        let xdg = tempfile::tempdir().expect("temp dir");
        let fallback = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            runtime_dir_from(Some(xdg.path().as_os_str()), fallback.path()),
            xdg.path()
        );
    }

    #[test]
    fn an_unset_xdg_runtime_dir_falls_back() {
        let fallback = tempfile::tempdir().expect("temp dir");
        assert_eq!(runtime_dir_from(None, fallback.path()), fallback.path());
    }

    #[test]
    fn an_empty_xdg_runtime_dir_falls_back() {
        let fallback = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            runtime_dir_from(Some(OsStr::new("")), fallback.path()),
            fallback.path()
        );
    }

    #[test]
    fn an_xdg_runtime_dir_that_is_not_a_directory_falls_back() {
        // A stale value must not send plaintext somewhere unwritable.
        let fallback = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            runtime_dir_from(Some(OsStr::new("/no/such/runtime/dir")), fallback.path()),
            fallback.path()
        );
    }

    #[test]
    fn a_temp_file_is_owner_only() {
        let file = SecureTempFile::create("note.md").expect("create");
        let mode = std::fs::metadata(file.path())
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);
    }

    #[test]
    fn a_temp_file_keeps_the_name_it_was_given() {
        // Editors pick syntax highlighting from the extension.
        let file = SecureTempFile::create("01ARZ-quarterly-review.md").expect("create");
        assert_eq!(
            file.path().file_name().expect("name"),
            "01ARZ-quarterly-review.md"
        );
    }

    #[test]
    fn a_temp_file_roundtrips_content() {
        let file = SecureTempFile::create("note.md").expect("create");
        let content = "---\ntitle: Über\r\n---\r\nBody — 日本語\n";
        file.write(content).expect("write");
        assert_eq!(file.read().expect("read"), content);
    }

    #[test]
    fn a_temp_file_is_gone_after_drop() {
        let path = {
            let file = SecureTempFile::create("note.md").expect("create");
            file.write("secret").expect("write");
            file.path().to_path_buf()
        };
        assert!(!path.exists());
    }

    #[test]
    fn keeping_a_temp_file_leaves_it_on_disk() {
        // The recovery route when an edit cannot be written back.
        let path = {
            let file = SecureTempFile::create("note.md").expect("create");
            file.write("work worth keeping").expect("write");
            file.keep()
        };
        assert!(path.exists());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "work worth keeping"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn two_temp_files_with_one_name_do_not_collide() {
        // Two concurrent edits of the same note each get their own directory.
        let first = SecureTempFile::create("note.md").expect("create");
        let second = SecureTempFile::create("note.md").expect("create");
        assert_ne!(first.path(), second.path());
    }
}
