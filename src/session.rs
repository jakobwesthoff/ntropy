// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A vault plus the means to read and write its notes (ADR 0041).
//!
//! [`Vault`] answers "where is everything"; a [`VaultSession`] adds "and how do
//! I read it". Operations take a session rather than a vault, so a plaintext
//! and an encrypted vault reach the same code with a different cipher behind
//! them.
//!
//! The session derefs to its vault, so every `session.layout()` and
//! `session.root()` reads exactly as it did when the parameter was a `&Vault`.

use std::sync::Arc;

use crate::cipher::{NoteCipher, PlaintextCipher, UnsupportedCipher};
use crate::vault::{Vault, layout};

/// A vault together with the cipher its notes are stored under.
#[derive(Debug, Clone)]
pub struct VaultSession {
    vault: Vault,
    cipher: Arc<dyn NoteCipher>,
}

impl VaultSession {
    /// A session over a vault whose notes are plain Markdown.
    pub fn plaintext(vault: Vault) -> Self {
        Self {
            vault,
            cipher: Arc::new(PlaintextCipher),
        }
    }

    /// A session over a vault with an explicitly supplied cipher.
    pub fn with_cipher(vault: Vault, cipher: Arc<dyn NoteCipher>) -> Self {
        Self { vault, cipher }
    }

    /// Open a session for a vault whose notes cannot be read here.
    ///
    /// What a build compiled without encryption support offers for an
    /// encrypted vault: the scanner still recognizes which files are notes, and
    /// every read reports the missing support once rather than failing per
    /// file.
    pub fn unsupported(vault: Vault) -> Self {
        Self {
            vault,
            cipher: Arc::new(UnsupportedCipher),
        }
    }

    /// The vault this session reads and writes.
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// The cipher mediating this session's note I/O.
    pub fn cipher(&self) -> &dyn NoteCipher {
        self.cipher.as_ref()
    }

    /// Whether this vault stores its notes encrypted.
    ///
    /// Asked by the handful of behaviours that genuinely differ by vault shape
    /// — views, filename realignment, link rewriting — rather than by anything
    /// that merely reads or writes a note, which the cipher already covers.
    pub fn is_encrypted(&self) -> bool {
        layout::is_encrypted(self.vault.root())
    }

    /// Whether notes can be read right now.
    ///
    /// False on an encrypted vault with no identity. Writing does not consult
    /// this: encrypting a new note needs only the public recipient, which is
    /// what lets `ntropy new` work on a locked vault.
    pub fn is_unlocked(&self) -> bool {
        self.cipher.can_read()
    }
}

impl std::ops::Deref for VaultSession {
    type Target = Vault;

    fn deref(&self) -> &Vault {
        &self.vault
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault_at(root: &std::path::Path) -> Vault {
        std::fs::create_dir_all(root.join(".ntropy")).expect("create .ntropy");
        std::fs::create_dir_all(root.join("all-notes")).expect("create all-notes");
        Vault::new(root)
    }

    #[test]
    fn a_plaintext_session_reads_markdown_and_is_unlocked() {
        let dir = tempfile::tempdir().expect("temp dir");
        let session = VaultSession::plaintext(vault_at(dir.path()));
        assert_eq!(session.cipher().extension(), "md");
        assert!(session.is_unlocked());
        assert!(!session.is_encrypted());
    }

    #[test]
    fn deref_exposes_the_vault_layout() {
        // The property that keeps every existing `vault.layout()` call site
        // compiling unchanged when its parameter becomes a session.
        let dir = tempfile::tempdir().expect("temp dir");
        let session = VaultSession::plaintext(vault_at(dir.path()));
        assert_eq!(session.layout().all_notes(), dir.path().join("all-notes"));
        assert_eq!(session.root(), dir.path());
        assert_eq!(session.vault().root(), dir.path());
    }

    #[test]
    fn a_recipient_file_makes_the_session_report_encrypted() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path());
        std::fs::write(dir.path().join(".ntropy/identity.pub"), "age1example\n")
            .expect("write recipient");

        // Detection is about the vault on disk, not about which cipher happens
        // to be attached, so even a plaintext session reports it. That is what
        // lets a build without encryption support notice and say so.
        assert!(VaultSession::plaintext(vault).is_encrypted());
    }

    #[test]
    fn an_unsupported_session_recognizes_notes_but_cannot_read_them() {
        let dir = tempfile::tempdir().expect("temp dir");
        let session = VaultSession::unsupported(vault_at(dir.path()));
        assert_eq!(session.cipher().extension(), "age");
        assert!(!session.is_unlocked());
    }
}
