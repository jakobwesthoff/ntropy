// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The record of a conversion in progress (ADR 0041).
//!
//! `.ntropy/migration.toml` exists only while a whole-vault conversion is under
//! way. Its presence makes every ordinary command refuse, so a half-converted
//! vault is never scanned, synced or edited further; its contents say which
//! conversion to finish and against which key.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::MigrateError;
use crate::fsutil;

/// The schema version of the marker file.
///
/// A future change to the file's shape can be told apart from a file this
/// version wrote, rather than being misread as one.
pub const MARKER_VERSION: u32 = 1;

/// Which conversion a marker describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    /// Plaintext notes to encrypted.
    Encrypt,
    /// Encrypted notes back to plaintext.
    Decrypt,
    /// Encrypted notes re-encrypted to a fresh keypair.
    Rekey,
}

impl Operation {
    /// The command that finishes an interrupted run of this operation.
    pub fn resume_command(self) -> &'static str {
        match self {
            Operation::Encrypt => "ntropy vault encrypt --resume",
            Operation::Decrypt => "ntropy vault decrypt --resume",
            Operation::Rekey => "ntropy vault rekey --resume",
        }
    }
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Operation::Encrypt => "encrypt",
            Operation::Decrypt => "decrypt",
            Operation::Rekey => "rekey",
        };
        f.write_str(name)
    }
}

/// A conversion in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// The marker schema version.
    pub version: u32,
    /// Which conversion is under way.
    pub operation: Operation,
    /// The recipient the notes are being converted *to*.
    ///
    /// Absent for `decrypt`, whose targets are plaintext.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// The recipient the sources were encrypted to.
    ///
    /// Recorded for `rekey`, where both ends are encrypted and a resume has to
    /// know which key still opens the sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_recipient: Option<String>,
    /// When the conversion began, for the reader of a stale marker.
    pub started: String,
}

impl Marker {
    /// A marker for a conversion starting now.
    pub fn new(operation: Operation, recipient: Option<String>, started: String) -> Self {
        Self {
            version: MARKER_VERSION,
            operation,
            recipient,
            source_recipient: None,
            started,
        }
    }
}

/// Read the marker at `path`, or `None` when no conversion is under way.
///
/// A malformed marker is an error rather than a silent `None`: a half-written
/// file means a conversion *was* running, and treating it as "nothing to see"
/// would let the next command scan a vault holding two copies of every note.
pub fn read_at(path: &Path) -> Result<Option<Marker>, MigrateError> {
    let Some(text) = fsutil::read_to_string_if_exists(path)? else {
        return Ok(None);
    };
    let marker: Marker = toml::from_str(&text).map_err(|source| MigrateError::MalformedMarker {
        path: path.to_path_buf(),
        source,
    })?;
    if marker.version != MARKER_VERSION {
        return Err(MigrateError::UnknownMarkerVersion {
            path: path.to_path_buf(),
            version: marker.version,
        });
    }
    Ok(Some(marker))
}

/// Write `marker` to `path`, durably.
///
/// Flushed before it is relied upon: the marker is what stops the next command
/// from touching a vault mid-conversion, and a marker that did not reach the
/// disk protects nothing.
pub fn write_at(path: &Path, marker: &Marker) -> Result<(), MigrateError> {
    let text = toml::to_string_pretty(marker)?;
    fsutil::atomic_write(path, text.as_bytes())?;
    fsutil::sync_file(path)?;
    if let Some(parent) = path.parent() {
        fsutil::sync_dir(parent)?;
    }
    Ok(())
}

/// Remove the marker, ending the conversion.
pub fn remove_at(path: &Path) -> Result<(), MigrateError> {
    if path.exists() {
        fsutil::remove_file(path)?;
    }
    Ok(())
}

/// The path of a vault's marker file.
pub fn path_for(layout: &crate::vault::Layout) -> PathBuf {
    layout.migration_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker() -> Marker {
        Marker::new(
            Operation::Encrypt,
            Some("age1example".into()),
            "2026-08-06T12:00:00Z".into(),
        )
    }

    #[test]
    fn a_marker_roundtrips_through_toml() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        write_at(&path, &marker()).expect("write");
        assert_eq!(read_at(&path).expect("read"), Some(marker()));
    }

    #[test]
    fn no_file_means_no_conversion() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            read_at(&dir.path().join("migration.toml")).expect("read"),
            None
        );
    }

    #[test]
    fn a_decrypt_marker_carries_no_recipient() {
        // Its targets are plaintext, so there is no key to record.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        let marker = Marker::new(Operation::Decrypt, None, "2026-08-06T12:00:00Z".into());
        write_at(&path, &marker).expect("write");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(!text.contains("recipient"), "got: {text}");
        assert_eq!(read_at(&path).expect("read"), Some(marker));
    }

    #[test]
    fn a_rekey_marker_records_both_keys() {
        // A resume has to know which key still opens the untouched sources.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        let mut marker = Marker::new(
            Operation::Rekey,
            Some("age1new".into()),
            "2026-08-06T12:00:00Z".into(),
        );
        marker.source_recipient = Some("age1old".into());
        write_at(&path, &marker).expect("write");
        assert_eq!(read_at(&path).expect("read"), Some(marker));
    }

    #[test]
    fn a_half_written_marker_is_an_error_not_a_silent_none() {
        // Treating it as "no conversion" would let the next command scan a
        // vault holding two copies of every note.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        std::fs::write(&path, "version = 1\noperation = \"enc").expect("write");
        assert!(matches!(
            read_at(&path),
            Err(MigrateError::MalformedMarker { .. })
        ));
    }

    #[test]
    fn an_empty_marker_file_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        std::fs::write(&path, "").expect("write");
        assert!(read_at(&path).is_err());
    }

    #[test]
    fn an_unknown_version_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        std::fs::write(
            &path,
            "version = 99\noperation = \"encrypt\"\nstarted = \"x\"\n",
        )
        .expect("write");
        assert!(matches!(
            read_at(&path),
            Err(MigrateError::UnknownMarkerVersion { version: 99, .. })
        ));
    }

    #[test]
    fn removing_is_idempotent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("migration.toml");
        write_at(&path, &marker()).expect("write");
        remove_at(&path).expect("remove");
        assert!(!path.exists());
        remove_at(&path).expect("remove again");
    }

    #[test]
    fn each_operation_names_its_own_resume_command() {
        for op in [Operation::Encrypt, Operation::Decrypt, Operation::Rekey] {
            let command = op.resume_command();
            assert!(command.contains(&op.to_string()), "{command}");
            assert!(command.contains("--resume"), "{command}");
        }
    }
}
