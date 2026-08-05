// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The note I/O seam (ADR 0041).
//!
//! Every note read and write in the crate goes through a [`NoteCipher`], so
//! encrypted and plaintext vaults share one code path and no caller has to ask
//! which kind of vault it is holding. The two implementations differ only in
//! what happens to the bytes and what the note files are called:
//! [`PlaintextCipher`] passes them through as `.md`, [`AgeCipher`] encrypts
//! them as `.age`.
//!
//! The trait, its error and [`Passphrase`] are always compiled. Only
//! [`AgeCipher`] sits behind the `encryption` feature, so a build without it
//! still recognizes an encrypted vault and reports that it cannot open one,
//! rather than mistaking the ciphertext for a corrupt note.

use std::path::Path;

use crate::fsutil::{self, FsError};

/// Why a note could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum CipherError {
    /// The underlying file operation failed.
    #[error(transparent)]
    Fs(#[from] FsError),

    /// The vault is encrypted and no identity is available, so nothing can be
    /// read. Writing is still possible; only reads reach this.
    #[error("this vault is locked; run `ntropy unlock`")]
    Locked,

    /// The vault is encrypted but this build has no encryption support.
    #[error(
        "this vault is encrypted, but this build of ntropy was compiled \
         without encryption support"
    )]
    Unsupported,

    /// The bytes on disk are not what this cipher expects: ciphertext handed
    /// to the plaintext reader, or a plaintext file handed to the age reader.
    #[error(transparent)]
    #[cfg(feature = "encryption")]
    Crypto(#[from] crate::crypto::CryptoError),

    /// A note file's bytes were not valid UTF-8.
    #[error("`{}` is not valid UTF-8", .0.display())]
    NotUtf8(std::path::PathBuf),
}

/// How a vault's notes are stored on disk.
///
/// `Send + Sync` because [`crate::scan`] hands the cipher to the parallel
/// directory walker's per-entry closures.
pub trait NoteCipher: Send + Sync + std::fmt::Debug {
    /// The note file extension, without the leading dot.
    fn extension(&self) -> &'static str;

    /// Whether reads are possible right now.
    ///
    /// False on an encrypted vault with no identity. Writes do not consult
    /// this: encrypting needs only the public recipient.
    fn can_read(&self) -> bool;

    /// Read and decode the note at `path`.
    fn read(&self, path: &Path) -> Result<String, CipherError>;

    /// Encode and write `content` to `path`, atomically.
    fn write(&self, path: &Path, content: &str) -> Result<(), CipherError>;
}

// =============================================================================
// Plaintext
// =============================================================================

/// Notes stored as Markdown, exactly as every ntropy vault stored them before
/// encryption existed.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlaintextCipher;

impl NoteCipher for PlaintextCipher {
    fn extension(&self) -> &'static str {
        "md"
    }

    fn can_read(&self) -> bool {
        true
    }

    fn read(&self, path: &Path) -> Result<String, CipherError> {
        let bytes = fsutil::read(path)?;
        String::from_utf8(bytes).map_err(|_| CipherError::NotUtf8(path.to_path_buf()))
    }

    fn write(&self, path: &Path, content: &str) -> Result<(), CipherError> {
        fsutil::atomic_write(path, content.as_bytes())?;
        Ok(())
    }
}

// =============================================================================
// Unsupported: encrypted vault, encryption compiled out
// =============================================================================

/// Stands in for an encrypted vault in a build without encryption support.
///
/// It reports the `.age` extension so the scanner still recognizes which files
/// are notes and does not warn about each one individually; both read and
/// write then fail with a message naming the missing feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedCipher;

impl NoteCipher for UnsupportedCipher {
    fn extension(&self) -> &'static str {
        "age"
    }

    fn can_read(&self) -> bool {
        false
    }

    fn read(&self, _path: &Path) -> Result<String, CipherError> {
        Err(CipherError::Unsupported)
    }

    fn write(&self, _path: &Path, _content: &str) -> Result<(), CipherError> {
        Err(CipherError::Unsupported)
    }
}

// =============================================================================
// age
// =============================================================================

#[cfg(feature = "encryption")]
mod age_cipher {
    use super::{CipherError, NoteCipher, Path};
    use crate::crypto::{Identity, Recipient, age_io};
    use crate::fsutil;

    /// Notes stored as individual age files encrypted to the vault recipient.
    ///
    /// The identity is optional, which is what encodes the asymmetric key
    /// model: a cipher built from the recipient alone writes notes but cannot
    /// read them, so note creation works on a locked vault.
    #[derive(Debug)]
    pub struct AgeCipher {
        recipient: Recipient,
        identity: Option<Identity>,
    }

    impl AgeCipher {
        /// Build a cipher that can write, and can read only if given an
        /// identity.
        pub fn new(recipient: Recipient, identity: Option<Identity>) -> Self {
            Self {
                recipient,
                identity,
            }
        }

        /// The recipient this cipher encrypts to.
        pub fn recipient(&self) -> &Recipient {
            &self.recipient
        }
    }

    impl NoteCipher for AgeCipher {
        fn extension(&self) -> &'static str {
            "age"
        }

        fn can_read(&self) -> bool {
            self.identity.is_some()
        }

        fn read(&self, path: &Path) -> Result<String, CipherError> {
            let identity = self.identity.as_ref().ok_or(CipherError::Locked)?;
            let ciphertext = fsutil::read(path)?;
            Ok(age_io::decrypt_with(identity, &ciphertext)?)
        }

        fn write(&self, path: &Path, content: &str) -> Result<(), CipherError> {
            let ciphertext = age_io::encrypt_to(&self.recipient, content)?;
            fsutil::atomic_write(path, &ciphertext)?;
            Ok(())
        }
    }
}

#[cfg(feature = "encryption")]
pub use age_cipher::AgeCipher;

// =============================================================================
// Passphrase
// =============================================================================

/// A passphrase held in memory.
///
/// Always compiled, because `ops::init` names it in a signature that exists in
/// both builds; it therefore cannot borrow age's `SecretString`.
///
/// `Drop` overwrites the buffer. That is a best-effort measure, not a
/// guarantee: a `String` that reallocated while being built may have left an
/// earlier copy behind, and the compiler is within its rights to elide a write
/// to memory it can prove is never read again. It costs nothing and removes
/// the obvious lingering copy.
#[derive(Clone, PartialEq, Eq)]
pub struct Passphrase(String);

impl Passphrase {
    /// Take ownership of a passphrase.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Read the passphrase.
    ///
    /// Named to make every call site read as a deliberate exposure.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether the passphrase carries no characters.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Passphrase {
    fn drop(&mut self) {
        // SAFETY of intent, not of unsafety: writing zero bytes over the
        // buffer keeps the passphrase out of freed memory. `as_mut_vec` is the
        // only way to reach the bytes, and zero is valid UTF-8, so the string
        // stays well-formed until it is dropped a moment later.
        unsafe {
            for byte in self.0.as_mut_vec() {
                std::ptr::write_volatile(byte, 0);
            }
        }
    }
}

impl std::fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Passphrase(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name in a directory, sorted, so a leftover temp sibling shows up.
    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    // -------------------------------------------------------------------------
    // Plaintext
    // -------------------------------------------------------------------------

    #[test]
    fn plaintext_cipher_reports_markdown_and_is_always_readable() {
        let cipher = PlaintextCipher;
        assert_eq!(cipher.extension(), "md");
        assert!(cipher.can_read());
    }

    #[test]
    fn plaintext_cipher_roundtrips_content_verbatim() {
        // Notes are reconstructed byte-for-byte from their retained header and
        // body, so the passthrough must not normalize line endings or encoding.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.md");
        let content = "---\ntitle: Über\r\n---\r\n\r\nBody — 日本語\n";

        let cipher = PlaintextCipher;
        cipher.write(&path, content).expect("write");
        assert_eq!(cipher.read(&path).expect("read"), content);
        assert_eq!(std::fs::read_to_string(&path).expect("raw read"), content);
    }

    #[test]
    fn plaintext_cipher_write_leaves_no_temp_siblings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.md");
        PlaintextCipher.write(&path, "body").expect("write");
        assert_eq!(entries(dir.path()), ["note.md"]);
    }

    #[test]
    fn plaintext_cipher_read_of_a_missing_file_is_a_filesystem_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let result = PlaintextCipher.read(&dir.path().join("no-such-note.md"));
        assert!(matches!(result, Err(CipherError::Fs(_))));
    }

    #[test]
    fn plaintext_cipher_read_of_non_utf8_names_the_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.md");
        std::fs::write(&path, [0xff, 0xfe]).expect("write bytes");

        match PlaintextCipher.read(&path) {
            Err(CipherError::NotUtf8(reported)) => assert_eq!(reported, path),
            other => panic!("expected NotUtf8, got {other:?}"),
        }
    }

    #[test]
    fn plaintext_cipher_write_replaces_existing_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.md");
        PlaintextCipher.write(&path, "first").expect("write");
        PlaintextCipher.write(&path, "second").expect("overwrite");
        assert_eq!(PlaintextCipher.read(&path).expect("read"), "second");
    }

    // -------------------------------------------------------------------------
    // Unsupported
    // -------------------------------------------------------------------------

    #[test]
    fn unsupported_cipher_claims_the_age_extension_but_cannot_read() {
        // Claiming the extension is deliberate: the scanner then recognizes
        // the files as notes and reports one clear error instead of warning
        // about every file in the vault.
        let cipher = UnsupportedCipher;
        assert_eq!(cipher.extension(), "age");
        assert!(!cipher.can_read());
    }

    #[test]
    fn unsupported_cipher_refuses_both_directions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("note.age");
        let cipher = UnsupportedCipher;

        assert!(matches!(cipher.read(&path), Err(CipherError::Unsupported)));
        assert!(matches!(
            cipher.write(&path, "body"),
            Err(CipherError::Unsupported)
        ));
        // A refused write must not have created anything.
        assert!(entries(dir.path()).is_empty());
    }

    #[test]
    fn unsupported_error_names_the_missing_feature() {
        let message = CipherError::Unsupported.to_string();
        assert!(message.contains("encrypted"), "{message}");
        assert!(message.contains("without encryption support"), "{message}");
    }

    #[test]
    fn locked_error_names_the_unlock_command() {
        let message = CipherError::Locked.to_string();
        assert!(message.contains("ntropy unlock"), "{message}");
    }

    // -------------------------------------------------------------------------
    // Passphrase
    // -------------------------------------------------------------------------

    #[test]
    fn passphrase_exposes_what_it_was_given() {
        let passphrase = Passphrase::new("correct horse");
        assert_eq!(passphrase.expose(), "correct horse");
        assert!(!passphrase.is_empty());
    }

    #[test]
    fn an_empty_passphrase_reports_itself_as_empty() {
        assert!(Passphrase::new("").is_empty());
    }

    #[test]
    fn passphrase_debug_hides_the_value() {
        let rendered = format!("{:?}", Passphrase::new("hunter2"));
        assert!(!rendered.contains("hunter2"), "{rendered}");
    }

    // -------------------------------------------------------------------------
    // age
    // -------------------------------------------------------------------------

    #[cfg(feature = "encryption")]
    mod age {
        use super::*;
        use crate::crypto::age_io;

        #[test]
        fn age_cipher_reports_the_age_extension() {
            let (_, recipient) = age_io::generate_keypair();
            assert_eq!(AgeCipher::new(recipient, None).extension(), "age");
        }

        #[test]
        fn age_cipher_writes_without_an_identity() {
            // The asymmetric property the locking model rests on: encrypting
            // needs only the public recipient, which is what lets `ntropy new`
            // work on a locked vault.
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            let (identity, recipient) = age_io::generate_keypair();

            let locked = AgeCipher::new(recipient, None);
            assert!(!locked.can_read());
            locked
                .write(&path, "created while locked\n")
                .expect("write");

            // The bytes really are the note, readable once a key shows up.
            let unlocked = AgeCipher::new(identity.to_public(), Some(identity));
            assert_eq!(
                unlocked.read(&path).expect("read"),
                "created while locked\n"
            );
        }

        #[test]
        fn age_cipher_read_without_an_identity_is_locked() {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            let (_, recipient) = age_io::generate_keypair();
            let cipher = AgeCipher::new(recipient, None);
            cipher.write(&path, "body").expect("write");

            assert!(matches!(cipher.read(&path), Err(CipherError::Locked)));
        }

        #[test]
        fn age_cipher_roundtrips_through_a_real_file() {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            let (identity, recipient) = age_io::generate_keypair();
            let cipher = AgeCipher::new(recipient, Some(identity));

            let content = "---\ntitle: Über Größe\r\n---\r\n\r\nBody — 日本語\n";
            cipher.write(&path, content).expect("write");
            assert_eq!(cipher.read(&path).expect("read"), content);
        }

        #[test]
        fn the_file_on_disk_is_not_the_note() {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            let (identity, recipient) = age_io::generate_keypair();
            AgeCipher::new(recipient, Some(identity))
                .write(&path, "title: Quarterly Review")
                .expect("write");

            let raw = std::fs::read(&path).expect("raw read");
            assert!(
                !raw.windows("Quarterly Review".len())
                    .any(|w| w == b"Quarterly Review")
            );
        }

        #[test]
        fn age_cipher_write_leaves_no_temp_siblings() {
            // A stray `.tmp` in an encrypted vault would be plaintext-adjacent
            // litter in a synced directory, so the atomic write must clean up.
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            let (_, recipient) = age_io::generate_keypair();
            AgeCipher::new(recipient, None)
                .write(&path, "body")
                .expect("write");

            assert_eq!(entries(dir.path()), ["note.age"]);
        }

        #[test]
        fn age_cipher_reading_a_plaintext_file_is_a_crypto_error() {
            // The hand-dropped-Markdown case: it must be a typed failure the
            // scanner can turn into a warning, not a panic or silent garbage.
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            std::fs::write(&path, "---\ntitle: Dropped In\n---\n").expect("write");

            let (identity, recipient) = age_io::generate_keypair();
            let cipher = AgeCipher::new(recipient, Some(identity));
            assert!(matches!(cipher.read(&path), Err(CipherError::Crypto(_))));
        }

        #[test]
        fn age_cipher_reading_with_the_wrong_identity_is_a_crypto_error() {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("note.age");
            let (_, recipient) = age_io::generate_keypair();
            AgeCipher::new(recipient, None)
                .write(&path, "body")
                .expect("write");

            let (other, other_recipient) = age_io::generate_keypair();
            let wrong = AgeCipher::new(other_recipient, Some(other));
            assert!(matches!(wrong.read(&path), Err(CipherError::Crypto(_))));
        }

        #[test]
        fn age_cipher_read_of_a_missing_file_is_a_filesystem_error() {
            let dir = tempfile::tempdir().expect("temp dir");
            let (identity, recipient) = age_io::generate_keypair();
            let cipher = AgeCipher::new(recipient, Some(identity));
            let result = cipher.read(&dir.path().join("no-such-note.age"));
            assert!(matches!(result, Err(CipherError::Fs(_))));
        }

        #[test]
        fn age_cipher_exposes_the_recipient_it_encrypts_to() {
            // The keychain is keyed by recipient, so callers must be able to
            // ask a cipher which key it belongs to.
            let (_, recipient) = age_io::generate_keypair();
            let expected = recipient.as_str();
            let cipher = AgeCipher::new(recipient, None);
            assert_eq!(cipher.recipient().as_str(), expected);
        }
    }
}
