// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Filesystem primitives: reads, atomic writes, renames, removals, symlinks,
//! and directory creation, listing and pruning.
//!
//! Every write ntropy makes inside a vault, and to the global config, goes
//! through here, except the built-in site theme that `site theme init` copies
//! into `.ntropy/themes/site/`. Every symlink is made here, so the Unix-only
//! symlink assumption lives in one place (ADRs 0008, 0020). Plain reads and
//! metadata lookups, private temporary files, and the files `render` and
//! `site` export to a destination the user names call `std::fs` directly.
//!
//! A write or rename here is on stable storage when it returns, so a power
//! loss cannot leave a note truncated (ADR 0079). A bulk operation opens a
//! [`FlushScope`] to flush once at its end instead of once per file.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::marker::PhantomData;
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

    /// The underlying I/O error, for an error type that carries the path and
    /// the failed step in its own terms.
    pub(crate) fn into_io(self) -> std::io::Error {
        self.source
    }

    /// Whether this is [`rename`] refusing a destination that already exists.
    ///
    /// The callers that rename notes turn exactly this case into a warning
    /// and carry on; every other failure stays an error.
    pub fn is_destination_taken(&self) -> bool {
        self.action == REFUSING_TO_REPLACE
    }
}

/// The action [`rename`] reports when the destination is taken.
const REFUSING_TO_REPLACE: &str = "refusing to replace";

type Result<T> = std::result::Result<T, FsError>;

// =============================================================================
// File reads
// =============================================================================

/// Read `path` as raw bytes.
///
/// Bytes rather than a string because a note file is not necessarily text: in
/// an encrypted vault it is age ciphertext, and decoding is the cipher's job
/// (ADR 0041).
pub fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| FsError::new("reading", path, e))
}

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

/// Write `contents` to `path` atomically and durably.
///
/// The bytes are first written to a uniquely named sibling temp file and then
/// renamed over the destination. `rename(2)` is atomic on a single filesystem,
/// so a reader either sees the old file or the fully written new one, never a
/// half-written note. The temp file is a sibling (same directory) so the rename
/// never crosses a filesystem boundary.
///
/// The temp file is flushed to stable storage before the rename and the
/// directory after it. Without the first flush a power loss can persist the
/// rename but not the data, leaving a truncated or empty note; without the
/// second the new name can be lost. A [`FlushScope`] postpones both flushes
/// for a bulk operation.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp_path = temp_sibling(path);
    std::fs::write(&tmp_path, contents).map_err(|e| FsError::new("writing", &tmp_path, e))?;
    place(&tmp_path, path)
}

/// Write `contents` to `path` atomically, readable only by its owner.
///
/// Same temp-then-rename shape as [`atomic_write`], with `0600` set on the
/// temp file *before* the rename. Setting it afterwards would leave a window
/// in which the finished file is world-readable, which for a wrapped vault
/// identity is the whole point of the mode (ADR 0041).
///
/// The wrapped vault identity is the only file that needs it, so it is
/// compiled only where that file can exist.
#[cfg(feature = "encryption")]
pub fn atomic_write_private(path: &Path, contents: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let tmp_path = temp_sibling(path);
    std::fs::write(&tmp_path, contents).map_err(|e| FsError::new("writing", &tmp_path, e))?;

    if let Err(e) = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600)) {
        discard(&tmp_path);
        return Err(FsError::new("setting permissions on", &tmp_path, e));
    }

    place(&tmp_path, path)
}

/// A uniquely named temp path beside `path`.
///
/// A ULID suffix keeps concurrent or repeated writes from clobbering each
/// other's temp file while still landing in the same directory.
fn temp_sibling(path: &Path) -> PathBuf {
    let tmp_name = match path.file_name() {
        Some(name) => format!("{}.{}.tmp", name.to_string_lossy(), Ulid::generate()),
        None => format!("{}.tmp", Ulid::generate()),
    };
    parent_dir(path).join(tmp_name)
}

/// The directory holding `path`: `.` for a bare file name, whose parent is
/// the empty path, which cannot be opened to flush.
fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Best-effort removal of a temp file that will not be placed. The failure
/// that made it unusable is the error worth reporting.
fn discard(tmp_path: &Path) {
    let _ = std::fs::remove_file(tmp_path);
}

/// Move the written temp file `tmp_path` over `destination` so that the data
/// is on stable storage before the name points at it.
///
/// Inside a [`FlushScope`] the placement waits for [`FlushScope::finish`],
/// which keeps this order for every file of the batch.
fn place(tmp_path: &Path, destination: &Path) -> Result<()> {
    let deferred = with_open_scope(|scope| {
        scope
            .placements
            .push_back((tmp_path.to_path_buf(), destination.to_path_buf()));
    });
    if deferred.is_some() {
        return Ok(());
    }

    sync_file(tmp_path).inspect_err(|_| discard(tmp_path))?;
    rename_raw(tmp_path, destination).map_err(|e| {
        discard(tmp_path);
        FsError::new("renaming into place", destination, e)
    })?;
    flush_directories(&[parent_dir(destination)])
}

/// Flush `path`'s contents all the way to stable storage.
///
/// On Apple targets `File::sync_all` is `fcntl(F_FULLFSYNC)`, which flushes
/// the drive's write cache; plain `fsync` there only hands the data to the
/// drive, and only the former survives losing power. Elsewhere it is `fsync`.
pub fn sync_file(path: &Path) -> Result<()> {
    let file = std::fs::File::open(path).map_err(|e| FsError::new("opening to flush", path, e))?;
    tolerate_unsupported(file.sync_all()).map_err(|e| FsError::new("flushing to disk", path, e))?;
    #[cfg(test)]
    trace::record(trace::Event::Flush(path.to_path_buf()));
    Ok(())
}

/// Flush a directory entry so the names inside it are durable.
///
/// Flushing a file makes its *contents* durable; the directory entry that
/// names it is separate metadata and needs its own flush, or a crash can lose
/// the fact that the file exists at all.
pub fn sync_dir(path: &Path) -> Result<()> {
    let dir = std::fs::File::open(path).map_err(|e| FsError::new("opening to flush", path, e))?;
    tolerate_unsupported(dir.sync_all())
        .map_err(|e| FsError::new("flushing directory to disk", path, e))?;
    #[cfg(test)]
    trace::record(trace::Event::FlushDir(path.to_path_buf()));
    Ok(())
}

/// Treat "this filesystem cannot flush" as success.
///
/// Mounts such as exFAT, SMB, NFS and FUSE can answer a flush with `EINVAL`,
/// `ENOTSUP` or `ENOTTY`. Failing every write there would leave the vault
/// unusable, so those mounts get no durability and no error. Every other
/// failure, such as `EIO` or `ENOSPC`, means the data may not be on disk and
/// stays an error.
fn tolerate_unsupported(result: std::io::Result<()>) -> std::io::Result<()> {
    match result {
        Err(e)
            if matches!(
                e.raw_os_error(),
                Some(libc::EINVAL | libc::ENOTSUP | libc::ENOTTY)
            ) =>
        {
            Ok(())
        }
        other => other,
    }
}

/// Make the names in `directories` durable now, or at the end of the open
/// [`FlushScope`].
fn flush_directories(directories: &[&Path]) -> Result<()> {
    let deferred = with_open_scope(|scope| {
        scope
            .directories
            .extend(directories.iter().map(|d| d.to_path_buf()));
    });
    if deferred.is_some() {
        return Ok(());
    }

    for (index, directory) in directories.iter().enumerate() {
        if !directories[..index].contains(directory) {
            sync_dir(directory)?;
        }
    }
    Ok(())
}

/// Make the existing file `path`, and the name it is listed under, durable.
///
/// For a file written earlier whose flush is still owed, such as a conversion
/// target that a resumed run found already in place. Inside a [`FlushScope`]
/// the flush waits for [`FlushScope::finish`].
///
/// Only the conversions resume, so it is compiled only where they exist.
#[cfg(feature = "encryption")]
pub fn make_durable(path: &Path) -> Result<()> {
    let deferred = with_open_scope(|scope| scope.files.push(path.to_path_buf()));
    if deferred.is_none() {
        sync_file(path)?;
    }
    flush_directories(&[parent_dir(path)])
}

// =============================================================================
// Deferred flushing for bulk operations
// =============================================================================

/// The writes of a bulk operation, held back so they can be flushed together.
///
/// Flushing after every write makes each one wait for the drive, which turns
/// a command that rewrites tens of thousands of notes from seconds into
/// minutes. The scope instead holds every written temp file back, and
/// [`FlushScope::finish`] flushes them all, then renames them, then flushes
/// each affected directory once. The order is what keeps a note whole after a
/// power loss at any point of the batch: a destination name only ever points
/// at data that was flushed before the rename, exactly as for a single write.
#[derive(Default)]
struct Deferred {
    /// Temp files written but not yet in place, as `(temp, destination)`, in
    /// the order they were written.
    placements: VecDeque<(PathBuf, PathBuf)>,
    /// Existing files whose contents must be flushed at the end.
    files: Vec<PathBuf>,
    /// Directories whose entries changed.
    directories: BTreeSet<PathBuf>,
}

impl Deferred {
    /// Flush the files, rename the temp files into place, flush the
    /// directories.
    fn commit(mut self) -> Result<()> {
        for path in self
            .placements
            .iter()
            .map(|(tmp, _)| tmp)
            .chain(&self.files)
        {
            sync_file(path)?;
        }
        while let Some((tmp_path, destination)) = self.placements.front() {
            rename_raw(tmp_path, destination)
                .map_err(|e| FsError::new("renaming into place", destination, e))?;
            self.directories
                .insert(parent_dir(destination).to_path_buf());
            self.placements.pop_front();
        }
        for directory in &self.directories {
            sync_dir(directory)?;
        }
        Ok(())
    }
}

impl Drop for Deferred {
    /// Temp files still waiting belong to a batch that did not finish; they
    /// hold content nobody asked to keep.
    fn drop(&mut self) {
        for (tmp_path, _) in &self.placements {
            discard(tmp_path);
        }
    }
}

thread_local! {
    static OPEN_SCOPE: RefCell<Option<Deferred>> = const { RefCell::new(None) };
}

/// Run `f` on the open scope of this thread, if there is one.
fn with_open_scope<T>(f: impl FnOnce(&mut Deferred) -> T) -> Option<T> {
    OPEN_SCOPE.with(|scope| scope.borrow_mut().as_mut().map(f))
}

/// Defers the flushes of every write on this thread until [`finish`].
///
/// Open one around a bulk operation and finish it before anything reads back
/// what the operation wrote: a deferred [`atomic_write`] leaves the old file
/// in place until then. Dropping the scope instead abandons the pending
/// writes and removes their temp files.
///
/// The state is per thread, and the guard cannot leave its thread, so a write
/// made elsewhere is never deferred; it flushes at once. A scope cannot be
/// opened inside another one.
///
/// [`finish`]: FlushScope::finish
#[derive(Debug)]
pub struct FlushScope {
    _not_send: PhantomData<*const ()>,
}

impl FlushScope {
    /// Start deferring on this thread.
    ///
    /// # Panics
    ///
    /// When a scope is already open on this thread.
    pub fn open() -> Self {
        OPEN_SCOPE.with(|scope| {
            let mut scope = scope.borrow_mut();
            assert!(
                scope.is_none(),
                "a flush scope is already open on this thread"
            );
            *scope = Some(Deferred::default());
        });
        Self {
            _not_send: PhantomData,
        }
    }

    /// Flush everything written since [`FlushScope::open`]: the temp files,
    /// then the renames into place, then each affected directory once.
    pub fn finish(self) -> Result<()> {
        let deferred = OPEN_SCOPE
            .with(|scope| scope.borrow_mut().take())
            .expect("the scope's state lives until the scope ends");
        deferred.commit()
    }
}

impl Drop for FlushScope {
    fn drop(&mut self) {
        // Dropping the taken state removes the temp files of an abandoned
        // batch. After `finish` there is nothing left to take.
        drop(OPEN_SCOPE.with(|scope| scope.borrow_mut().take()));
    }
}

/// Remove `dir` and everything under it.
///
/// The one place ntropy deletes a directory tree: a conversion to encrypted
/// storage removes the materialized views, which an encrypted vault cannot
/// have. Every other path leaves directories for the user (ADR 0018).
#[cfg(feature = "encryption")]
pub fn remove_dir_all(dir: &Path) -> Result<()> {
    std::fs::remove_dir_all(dir).map_err(|e| FsError::new("removing directory tree", dir, e))
}

/// Rename `from` to `to`, refusing to replace anything already at `to`, and
/// flush the directories involved so the new name survives a power loss.
///
/// `rename(2)` silently replaces an existing destination, which for a note
/// file deletes another note's content (ADR 0067). The one existing
/// destination admitted is `from` itself under another spelling: on a
/// case-insensitive filesystem such as default APFS, `X-note.md` and
/// `x-note.md` name the same entry, and renaming between them only changes
/// how the name is written. That case is recognized by identity, the same
/// device and inode with a single link, rather than by comparing names, so
/// it covers whatever the filesystem folds (case, Unicode normalization)
/// and still refuses two hard links of one file, between which `rename(2)`
/// would succeed without doing anything. A file that also has a hard link
/// elsewhere has two links, so even its case-only rename is refused.
///
/// The check and the rename are two calls. A second process creating that
/// exact name in between is not guarded against. An atomic no-replace rename
/// exists only per platform (`renamex_np`, `renameat2`), with fallbacks for
/// filesystems that do not support it. Linking and then unlinking needs
/// filesystems with hard links, which exFAT and some network mounts lack.
/// Both would still need the same-entry exception above.
pub fn rename(from: &Path, to: &Path) -> Result<()> {
    match std::fs::symlink_metadata(to) {
        Ok(existing) if !is_same_entry(from, &existing) => {
            return Err(FsError::new(
                REFUSING_TO_REPLACE,
                to,
                std::io::Error::from(std::io::ErrorKind::AlreadyExists),
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(FsError::new("checking", to, e)),
    }
    rename_raw(from, to).map_err(|e| FsError::new("renaming", from, e))?;
    flush_directories(&[parent_dir(to), parent_dir(from)])
}

/// The one place a file is renamed, so the test trace sees every rename.
fn rename_raw(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)?;
    #[cfg(test)]
    trace::record(trace::Event::Rename {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
    });
    Ok(())
}

/// A file's identity: its device and inode. Two paths with the same
/// identity name one file, whatever symlinks or letter case lead there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId {
    device: u64,
    inode: u64,
}

/// The identity of the file `path` names, following symlinks, or `None`
/// when it cannot be read.
pub fn file_id(path: &Path) -> Option<FileId> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path).ok()?;
    Some(FileId {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

/// Whether `existing`, the metadata found at a rename's destination, is the
/// entry `from` names.
fn is_same_entry(from: &Path, existing: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    std::fs::symlink_metadata(from).is_ok_and(|source| {
        source.dev() == existing.dev() && source.ino() == existing.ino() && existing.nlink() == 1
    })
}

/// Remove `path` if it is a regular file or symlink.
pub fn remove_file(path: &Path) -> Result<()> {
    std::fs::remove_file(path).map_err(|e| FsError::new("removing file", path, e))?;
    #[cfg(test)]
    trace::record(trace::Event::Remove(path.to_path_buf()));
    Ok(())
}

/// Remove `path` and flush its directory, so that the removal cannot be
/// undone by a power loss.
///
/// For a file whose other copy has just been made durable: a removal that
/// comes back after a crash leaves two notes where one was meant. Inside a
/// [`FlushScope`] the directory is flushed by [`FlushScope::finish`].
pub fn remove_file_durably(path: &Path) -> Result<()> {
    remove_file(path)?;
    flush_directories(&[parent_dir(path)])
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
    remove_empty_dir(path)
}

/// Remove the directory `path`, which was empty when last looked at.
///
/// Another process, such as the language server syncing views on save, can
/// add an entry or remove the directory between that look and this removal,
/// so both outcomes are the same no-op the emptiness check would have chosen.
fn remove_empty_dir(path: &Path) -> Result<()> {
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
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

/// A recording of the flushes, renames and removals the primitives make, so
/// tests can pin their order. A power loss cannot be simulated, but the order
/// of these calls is what decides whether a note survives one.
///
/// The recording is per thread: a write made on another thread is missing from
/// it, which is how the tests notice a bulk write that left its thread.
#[cfg(test)]
pub(crate) mod trace {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    pub(crate) enum Event {
        Flush(PathBuf),
        FlushDir(PathBuf),
        Rename { from: PathBuf, to: PathBuf },
        Remove(PathBuf),
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn record(event: Event) {
        EVENTS.with(|events| events.borrow_mut().push(event));
    }

    /// Run `f` and return its result with the events it caused, one line each:
    /// `flush <path>`, `flush-dir <path>`, `rename <from> -> <to>`,
    /// `remove <path>`. Paths are relative to `root` (`.` for `root` itself)
    /// and a temp file's `.<ULID>.tmp` suffix reads as `.tmp`.
    pub(crate) fn capture<T>(root: &Path, f: impl FnOnce() -> T) -> (T, Vec<String>) {
        let outer = EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()));
        let result = f();
        let events = EVENTS.with(|events| std::mem::replace(&mut *events.borrow_mut(), outer));
        (result, events.iter().map(|e| describe(root, e)).collect())
    }

    fn describe(root: &Path, event: &Event) -> String {
        match event {
            Event::Flush(path) => format!("flush {}", short(root, path)),
            Event::FlushDir(path) => format!("flush-dir {}", short(root, path)),
            Event::Rename { from, to } => {
                format!("rename {} -> {}", short(root, from), short(root, to))
            }
            Event::Remove(path) => format!("remove {}", short(root, path)),
        }
    }

    fn short(root: &Path, path: &Path) -> String {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let text = relative.display().to_string();
        let text = if text.is_empty() {
            ".".to_string()
        } else {
            text
        };
        match text.strip_suffix(".tmp").and_then(|s| s.rsplit_once('.')) {
            Some((stem, ulid)) if ulid.len() == 26 => format!("{stem}.tmp"),
            _ => text,
        }
    }
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
    fn remove_dir_reports_directory_not_empty_for_a_populated_dir() {
        // The error kind `remove_empty_dir` relies on, as macOS and Linux
        // report it (ADR 0020).
        let dir = tempfile::tempdir().expect("temp dir");
        let group = dir.path().join("group");
        std::fs::create_dir(&group).expect("mkdir");
        std::fs::write(group.join("a.md"), b"x").expect("seed");
        let err = std::fs::remove_dir(&group).expect_err("populated");
        assert_eq!(err.kind(), std::io::ErrorKind::DirectoryNotEmpty);
    }

    #[test]
    fn remove_empty_dir_leaves_a_dir_that_became_populated() {
        // The state a concurrent view sync leaves when it adds an entry after
        // the emptiness check: the removal must still be a no-op.
        let dir = tempfile::tempdir().expect("temp dir");
        let group = dir.path().join("group");
        std::fs::create_dir(&group).expect("mkdir");
        std::fs::write(group.join("a.md"), b"x").expect("seed");
        remove_empty_dir(&group).expect("a populated directory is not an error");
        assert!(group.join("a.md").is_file());
    }

    #[test]
    fn remove_empty_dir_reports_any_other_failure() {
        // A regular file is not a directory to remove; only an entry or a
        // missing directory is tolerated.
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("a.md");
        std::fs::write(&file, b"x").expect("seed");
        let err = remove_empty_dir(&file).expect_err("not a directory");
        assert!(err.to_string().contains("removing directory"), "{err}");
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

    #[test]
    fn rename_refuses_an_existing_destination() {
        // Replacing the destination would delete whatever it held, which for
        // a note is its content.
        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("old.md");
        let to = dir.path().join("taken.md");
        std::fs::write(&from, b"moving").expect("seed from");
        std::fs::write(&to, b"already here").expect("seed to");

        let err = rename(&from, &to).expect_err("the destination is taken");

        assert!(
            err.to_string().contains("refusing to replace"),
            "message was: {err}"
        );
        assert!(err.is_destination_taken());
        assert_eq!(std::fs::read(&from).expect("read from"), b"moving");
        assert_eq!(std::fs::read(&to).expect("read to"), b"already here");
    }

    #[test]
    fn rename_refuses_a_hard_link_under_another_name() {
        // `rename(2)` between two links of one file succeeds without doing
        // anything, so both names would stay while the caller believed the
        // old one gone.
        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("a.md");
        let to = dir.path().join("b.md");
        std::fs::write(&from, b"x").expect("seed");
        std::fs::hard_link(&from, &to).expect("hard link");

        let err = rename(&from, &to).expect_err("the destination is another link");

        assert!(
            err.to_string().contains("refusing to replace"),
            "message was: {err}"
        );
        assert!(from.exists() && to.exists());
    }

    #[test]
    fn rename_changes_only_the_case_of_a_name() {
        // On a case-insensitive filesystem the new spelling already "exists":
        // it is the file being renamed. Elsewhere it is an ordinary rename.
        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("note.md");
        let to = dir.path().join("NOTE.md");
        std::fs::write(&from, b"x").expect("seed");

        rename(&from, &to).expect("a case-only rename");

        let names: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["NOTE.md"]);
    }

    #[test]
    fn rename_reports_a_destination_it_cannot_inspect() {
        // A parent without search permission hides whether the destination
        // exists; that is an error, not a free name.
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("note.md");
        std::fs::write(&from, b"x").expect("seed");
        let sealed = dir.path().join("sealed");
        std::fs::create_dir(&sealed).expect("mkdir");
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000)).expect("chmod");

        let result = rename(&from, &sealed.join("note.md"));

        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o755))
            .expect("restore permissions for cleanup");
        let err = result.expect_err("the destination cannot be inspected");
        assert!(err.to_string().contains("checking"), "message was: {err}");
        assert!(!err.is_destination_taken());
        assert!(from.exists());
    }

    #[test]
    fn the_trace_lists_renames_and_removals_in_the_order_they_happen() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = dir.path().join("first.md");
        std::fs::write(&first, b"x").expect("seed");

        let ((), events) = trace::capture(dir.path(), || {
            rename(&first, &dir.path().join("second.md")).expect("rename");
            remove_file(&dir.path().join("second.md")).expect("remove");
        });

        assert_eq!(
            events,
            [
                "rename first.md -> second.md",
                "flush-dir .",
                "remove second.md"
            ]
        );
    }

    #[test]
    fn the_trace_names_a_temp_file_without_its_ulid() {
        let dir = tempfile::tempdir().expect("temp dir");

        let ((), events) = trace::capture(dir.path(), || {
            atomic_write(&dir.path().join("note.md"), b"x").expect("write");
        });

        assert_eq!(
            events,
            [
                "flush note.md.tmp",
                "rename note.md.tmp -> note.md",
                "flush-dir ."
            ]
        );
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn the_trace_lists_flushes_of_files_and_directories() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("note.md");
        std::fs::write(&file, b"x").expect("seed");

        let ((), events) = trace::capture(dir.path(), || {
            sync_file(&file).expect("flush file");
            sync_dir(dir.path()).expect("flush directory");
        });

        assert_eq!(events, ["flush note.md", "flush-dir ."]);
    }

    #[test]
    fn a_trace_does_not_include_events_from_before_or_after_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let note = dir.path().join("note.md");
        std::fs::write(&note, b"x").expect("seed");
        remove_file(&note).expect("remove before");

        let ((), events) = trace::capture(dir.path(), || {});

        assert!(events.is_empty(), "events were: {events:?}");
    }

    // ---- durable writes -------------------------------------------------

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn atomic_write_flushes_the_file_then_renames_then_flushes_the_directory() {
        let dir = temp_dir();

        let (result, events) = trace::capture(dir.path(), || {
            atomic_write(&dir.path().join("note.md"), b"x")
        });

        result.expect("write");
        assert_eq!(
            events,
            [
                "flush note.md.tmp",
                "rename note.md.tmp -> note.md",
                "flush-dir ."
            ]
        );
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn atomic_write_private_flushes_like_atomic_write_and_keeps_the_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir();
        let path = dir.path().join("identity.age");

        let (result, events) = trace::capture(dir.path(), || atomic_write_private(&path, b"x"));

        result.expect("write");
        assert_eq!(
            events,
            [
                "flush identity.age.tmp",
                "rename identity.age.tmp -> identity.age",
                "flush-dir ."
            ]
        );
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn rename_flushes_the_directory_after_renaming() {
        let dir = temp_dir();
        let from = dir.path().join("a.md");
        std::fs::write(&from, b"x").expect("seed");

        let (result, events) =
            trace::capture(dir.path(), || rename(&from, &dir.path().join("b.md")));

        result.expect("rename");
        assert_eq!(events, ["rename a.md -> b.md", "flush-dir ."]);
    }

    #[test]
    fn rename_across_directories_flushes_both() {
        let dir = temp_dir();
        std::fs::create_dir(dir.path().join("sub")).expect("mkdir");
        let from = dir.path().join("sub").join("a.md");
        std::fs::write(&from, b"x").expect("seed");

        let (result, events) =
            trace::capture(dir.path(), || rename(&from, &dir.path().join("a.md")));

        result.expect("rename");
        assert_eq!(
            events,
            ["rename sub/a.md -> a.md", "flush-dir .", "flush-dir sub"]
        );
    }

    #[test]
    fn rename_refusing_a_taken_destination_flushes_nothing() {
        let dir = temp_dir();
        let from = dir.path().join("a.md");
        let taken = dir.path().join("b.md");
        std::fs::write(&from, b"x").expect("seed");
        std::fs::write(&taken, b"y").expect("seed");

        let (result, events) = trace::capture(dir.path(), || rename(&from, &taken));

        assert!(result.is_err());
        assert!(events.is_empty(), "events were: {events:?}");
    }

    #[test]
    fn atomic_write_removes_the_temp_file_when_the_rename_fails() {
        let dir = temp_dir();
        let destination = dir.path().join("note.md");
        std::fs::create_dir(&destination).expect("mkdir");
        std::fs::write(destination.join("inside"), b"x").expect("seed");

        let (result, _) = trace::capture(dir.path(), || atomic_write(&destination, b"x"));

        assert!(result.is_err(), "a non-empty directory cannot be replaced");
        assert_eq!(names(dir.path()), ["note.md"]);
    }

    // ---- the deferred flush scope ---------------------------------------

    #[test]
    fn a_scope_defers_every_flush_until_it_finishes() {
        let dir = temp_dir();
        let scope = FlushScope::open();

        let (result, during) = trace::capture(dir.path(), || {
            atomic_write(&dir.path().join("a.md"), b"a")?;
            atomic_write(&dir.path().join("b.md"), b"b")
        });
        result.expect("writes");
        let (finished, at_finish) = trace::capture(dir.path(), || scope.finish());

        finished.expect("finish");
        assert!(during.is_empty(), "events while open: {during:?}");
        assert_eq!(
            at_finish,
            [
                "flush a.md.tmp",
                "flush b.md.tmp",
                "rename a.md.tmp -> a.md",
                "rename b.md.tmp -> b.md",
                "flush-dir ."
            ]
        );
    }

    #[test]
    fn a_scope_flushes_each_directory_once() {
        let dir = temp_dir();
        std::fs::create_dir(dir.path().join("sub")).expect("mkdir");
        let scope = FlushScope::open();
        atomic_write(&dir.path().join("y.md"), b"y").expect("write");
        atomic_write(&dir.path().join("sub").join("x.md"), b"x").expect("write");
        atomic_write(&dir.path().join("z.md"), b"z").expect("write");

        let (finished, events) = trace::capture(dir.path(), || scope.finish());

        finished.expect("finish");
        let flushed: Vec<&String> = events
            .iter()
            .filter(|e| e.starts_with("flush-dir"))
            .collect();
        assert_eq!(flushed, ["flush-dir .", "flush-dir sub"]);
    }

    #[test]
    fn a_scope_renames_only_after_flushing_every_file() {
        let dir = temp_dir();
        let scope = FlushScope::open();
        for name in ["a", "b", "c", "d", "e"] {
            atomic_write(&dir.path().join(format!("{name}.md")), b"x").expect("write");
        }

        let (finished, events) = trace::capture(dir.path(), || scope.finish());

        finished.expect("finish");
        let last_flush = events
            .iter()
            .rposition(|e| e.starts_with("flush "))
            .expect("flush");
        let first_rename = events
            .iter()
            .position(|e| e.starts_with("rename "))
            .expect("rename");
        assert!(last_flush < first_rename, "events were: {events:?}");
        assert_eq!(events.iter().filter(|e| e.starts_with("flush ")).count(), 5);
    }

    #[test]
    fn a_scope_leaves_the_destinations_old_until_it_finishes() {
        let dir = temp_dir();
        let path = dir.path().join("note.md");
        std::fs::write(&path, b"old").expect("seed");
        let scope = FlushScope::open();

        atomic_write(&path, b"new").expect("write");
        assert_eq!(std::fs::read(&path).expect("read"), b"old");
        scope.finish().expect("finish");

        assert_eq!(std::fs::read(&path).expect("read"), b"new");
    }

    #[test]
    fn dropping_a_scope_removes_pending_temp_files() {
        let dir = temp_dir();
        let scope = FlushScope::open();
        atomic_write(&dir.path().join("a.md"), b"a").expect("write");

        let ((), events) = trace::capture(dir.path(), || drop(scope));

        assert!(events.is_empty(), "events were: {events:?}");
        assert!(
            names(dir.path()).is_empty(),
            "left: {:?}",
            names(dir.path())
        );
    }

    #[test]
    fn a_dropped_scope_lets_the_next_write_flush_at_once() {
        let dir = temp_dir();
        drop(FlushScope::open());

        let (result, events) =
            trace::capture(dir.path(), || atomic_write(&dir.path().join("a.md"), b"a"));

        result.expect("write");
        assert_eq!(events.len(), 3, "events were: {events:?}");
    }

    #[test]
    fn a_failed_finish_removes_the_temps_it_did_not_rename() {
        let dir = temp_dir();
        let blocked = dir.path().join("b.md");
        std::fs::create_dir(&blocked).expect("mkdir");
        std::fs::write(blocked.join("inside"), b"x").expect("seed");
        let scope = FlushScope::open();
        atomic_write(&dir.path().join("a.md"), b"a").expect("write");
        atomic_write(&blocked, b"b").expect("write is deferred");
        atomic_write(&dir.path().join("c.md"), b"c").expect("write");

        let result = scope.finish();

        assert!(result.is_err());
        assert_eq!(names(dir.path()), ["a.md", "b.md"]);
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn make_durable_flushes_the_file_and_its_directory() {
        let dir = temp_dir();
        let path = dir.path().join("note.md");
        std::fs::write(&path, b"x").expect("seed");

        let (result, events) = trace::capture(dir.path(), || make_durable(&path));

        result.expect("flush");
        assert_eq!(events, ["flush note.md", "flush-dir ."]);
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn make_durable_in_a_scope_waits_for_finish() {
        let dir = temp_dir();
        let path = dir.path().join("note.md");
        std::fs::write(&path, b"x").expect("seed");
        let scope = FlushScope::open();

        let (result, during) = trace::capture(dir.path(), || make_durable(&path));
        result.expect("register");
        let (finished, at_finish) = trace::capture(dir.path(), || scope.finish());

        finished.expect("finish");
        assert!(during.is_empty(), "events while open: {during:?}");
        assert_eq!(at_finish, ["flush note.md", "flush-dir ."]);
    }

    #[test]
    fn remove_file_durably_flushes_the_directory_after_removing() {
        let dir = temp_dir();
        let path = dir.path().join("note.md");
        std::fs::write(&path, b"x").expect("seed");

        let (result, events) = trace::capture(dir.path(), || remove_file_durably(&path));

        result.expect("remove");
        assert_eq!(events, ["remove note.md", "flush-dir ."]);
    }

    #[test]
    fn remove_file_durably_in_a_scope_flushes_the_directory_at_finish() {
        let dir = temp_dir();
        let path = dir.path().join("note.md");
        std::fs::write(&path, b"x").expect("seed");
        let scope = FlushScope::open();

        let (result, during) = trace::capture(dir.path(), || remove_file_durably(&path));
        result.expect("remove");
        let (finished, at_finish) = trace::capture(dir.path(), || scope.finish());

        finished.expect("finish");
        assert_eq!(during, ["remove note.md"]);
        assert_eq!(at_finish, ["flush-dir ."]);
    }

    #[test]
    fn a_scope_with_nothing_pending_flushes_nothing() {
        let dir = temp_dir();
        let scope = FlushScope::open();

        let (finished, events) = trace::capture(dir.path(), || scope.finish());

        finished.expect("finish");
        assert!(events.is_empty(), "events were: {events:?}");
    }

    #[test]
    #[should_panic(expected = "already open")]
    fn opening_a_scope_inside_a_scope_panics() {
        let _outer = FlushScope::open();
        let _inner = FlushScope::open();
    }

    #[test]
    fn a_scope_does_not_defer_writes_of_other_threads() {
        let dir = temp_dir();
        let scope = FlushScope::open();
        let root = dir.path().to_path_buf();
        let (sender, receiver) = std::sync::mpsc::channel();

        let writer = std::thread::spawn(move || {
            let (result, events) = trace::capture(&root, || atomic_write(&root.join("x.md"), b"x"));
            sender
                .send((result.is_ok(), events))
                .expect("the receiver outlives the thread");
        });
        let (ok, events) = receiver.recv().expect("the thread reports");
        writer.join().expect("thread ends");

        assert!(ok);
        assert_eq!(
            events,
            ["flush x.md.tmp", "rename x.md.tmp -> x.md", "flush-dir ."]
        );
        drop(scope);
    }

    #[test]
    fn a_bare_file_name_lives_in_the_current_directory() {
        // `Path::new("note.md").parent()` is the empty path, which cannot be
        // opened to flush; a file without a name, such as `/`, has none at all.
        assert_eq!(parent_dir(Path::new("note.md")), Path::new("."));
        assert_eq!(parent_dir(Path::new("/")), Path::new("."));
        assert_eq!(parent_dir(Path::new("notes/note.md")), Path::new("notes"));
    }

    #[test]
    fn a_temp_file_lands_beside_its_destination() {
        let named = temp_sibling(Path::new("notes/note.md"));
        assert_eq!(named.parent(), Some(Path::new("notes")));
        let file = named.file_name().expect("name").to_string_lossy();
        assert!(
            file.starts_with("note.md.") && file.ends_with(".tmp"),
            "{file}"
        );

        let nameless = temp_sibling(Path::new("/"));
        assert!(nameless.to_string_lossy().ends_with(".tmp"));
    }

    #[test]
    fn only_unsupported_flush_errors_are_tolerated() {
        for tolerated in [libc::EINVAL, libc::ENOTSUP, libc::ENOTTY] {
            let result = tolerate_unsupported(Err(std::io::Error::from_raw_os_error(tolerated)));
            assert!(result.is_ok(), "errno {tolerated} must be tolerated");
        }
        for fatal in [libc::EIO, libc::ENOSPC, libc::EBADF] {
            let result = tolerate_unsupported(Err(std::io::Error::from_raw_os_error(fatal)));
            assert!(result.is_err(), "errno {fatal} must be an error");
        }
        assert!(tolerate_unsupported(Ok(())).is_ok());
    }
}
