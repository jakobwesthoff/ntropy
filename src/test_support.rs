// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Shared fixtures for the library's unit tests.
//!
//! Several modules exercise behavior against a real on-disk vault (view
//! materialization, reconciliation, deletion). They all need the same scaffold:
//! a temporary vault with `all-notes/`, a `.ntropy/` config, and one or more
//! configured views. These helpers build that scaffold so each test module does
//! not carry its own copy.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::config::{PerVaultConfig, ViewConfig};
use crate::session::VaultSession;
use crate::vault::Vault;

/// Build a temporary vault with `all-notes/` and the given `(name, field)`
/// views configured.
///
/// The returned [`TempDir`] guards the vault's lifetime: keep it bound for the
/// duration of the test, as dropping it removes the directory tree.
pub(crate) fn vault_with_views(views: &[(&str, &str)]) -> (TempDir, VaultSession) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");

    let mut config = PerVaultConfig::default();
    for (name, field) in views {
        config.add(ViewConfig {
            name: (*name).into(),
            field: (*field).into(),
        });
    }
    std::fs::write(
        root.join(".ntropy/config.toml"),
        config.to_toml().expect("toml"),
    )
    .expect("write config");

    let session = VaultSession::plaintext(Vault::new(root));
    (dir, session)
}

/// Build a temporary vault with a single `by-tag` view grouping by `tags`.
pub(crate) fn vault_with_view() -> (TempDir, VaultSession) {
    vault_with_views(&[("by-tag", "tags")])
}

/// Write `content` to `all-notes/<name>` and return the note's path.
///
/// Writes the bytes verbatim, so in an encrypted vault this is how a test
/// plants a stray plaintext file for `reconcile` to adopt. Use
/// [`write_encrypted_note`] to plant a proper one.
pub(crate) fn write_note(session: &VaultSession, name: &str, content: &str) -> PathBuf {
    let path = session.layout().all_notes().join(name);
    std::fs::write(&path, content).expect("write note");
    path
}

/// A valid view name whose directory under `root` cannot be inspected,
/// whatever user the test runs as.
///
/// Real directories are created below `root` until the path is a few bytes
/// short of the platform's `PATH_MAX`; the returned name is that path plus
/// one more segment, so inspecting it fails with `ENAMETOOLONG`. A
/// permission-based setup would not do: as root, permissions do not stop a
/// lookup.
pub(crate) fn uninspectable_view(root: &Path) -> crate::vault::ViewName {
    let path_max = usize::try_from(libc::PATH_MAX).expect("PATH_MAX is positive");
    // Short of the limit, so creating the prefix succeeds, but closer to it
    // than one more segment, so the next lookup exceeds it.
    let target = path_max - 8;
    let full = "a".repeat(crate::text::slug::MAX_SLUG_LEN);

    let mut segments: Vec<String> = Vec::new();
    let mut length = root.as_os_str().len();
    while length < target {
        let room = target - length - 1;
        let segment = if room >= full.len() {
            full.clone()
        } else {
            "a".repeat(room.max(1))
        };
        length += segment.len() + 1;
        segments.push(segment);
    }
    let prefix = segments.join("/");
    std::fs::create_dir_all(root.join(&prefix)).expect("create the long prefix");

    let name = format!("{prefix}/{full}");
    crate::vault::ViewName::parse(&name).expect("every segment is a valid slug")
}

/// Fixtures for vaults whose notes are encrypted.
///
/// Every encrypted fixture is built at test setup from a freshly generated
/// keypair rather than committed: age ciphertext is randomized per file, so it
/// is never snapshot-stable and a checked-in fixture would be unreadable to a
/// reviewer (ADR 0041).
#[cfg(feature = "encryption")]
pub(crate) mod encrypted {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;

    use crate::cipher::AgeCipher;
    use crate::crypto::{Identity, age_io};
    use crate::session::VaultSession;
    use crate::vault::Vault;

    /// An encrypted vault plus the identity that opens it.
    pub(crate) struct EncryptedVault {
        /// Guards the vault's lifetime; keep it bound for the test's duration.
        pub(crate) _dir: TempDir,
        /// An unlocked session over the vault.
        pub(crate) session: VaultSession,
        /// The vault's identity, for building a second session by hand.
        pub(crate) identity: Identity,
    }

    impl EncryptedVault {
        /// A session over the same vault with no identity, standing for a
        /// machine where `ntropy unlock` has not run.
        pub(crate) fn locked(&self) -> VaultSession {
            let recipient = self.identity.to_public();
            VaultSession::with_cipher(
                Vault::new(self.session.root()),
                Arc::new(AgeCipher::new(recipient, None)),
            )
        }
    }

    /// Build an encrypted vault: `all-notes/`, `.ntropy/` and a recipient file.
    pub(crate) fn encrypted_vault() -> EncryptedVault {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");

        let (identity, recipient) = age_io::generate_keypair();
        // The recipient file is what marks the vault encrypted, so detection
        // works exactly as it does for a real vault.
        std::fs::write(
            root.join(".ntropy/identity.pub"),
            format!("{}\n", recipient.as_str()),
        )
        .expect("write recipient");

        let session = VaultSession::with_cipher(
            Vault::new(root),
            Arc::new(AgeCipher::new(recipient, Some(identity.clone()))),
        );
        EncryptedVault {
            _dir: dir,
            session,
            identity,
        }
    }

    /// [`encrypted_vault`] with the identity also wrapped under `passphrase`
    /// in `.ntropy/identity.age`, as `vault encrypt` leaves it.
    pub(crate) fn encrypted_vault_with_wrapped_identity(passphrase: &str) -> EncryptedVault {
        let vault = encrypted_vault();
        let layout = vault.session.layout();
        crate::keys::write_keypair_at(
            &layout.identity_pub(),
            &layout.identity_file(),
            &vault.identity,
            &crate::cipher::Passphrase::new(passphrase),
        )
        .expect("write the vault keypair");
        vault
    }

    /// Encrypt `content` into `all-notes/<ulid>.age` and return its path.
    pub(crate) fn write_encrypted_note(
        session: &VaultSession,
        ulid: &str,
        content: &str,
    ) -> PathBuf {
        let path = session.layout().all_notes().join(format!("{ulid}.age"));
        session.cipher().write(&path, content).expect("write note");
        path
    }
}
