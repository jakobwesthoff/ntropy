// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Readable copies of encrypted notes, for following a link (ADR 0041).
//!
//! Everything that hands the editor a note's location — goto-definition,
//! document links, workspace symbols — would otherwise point at ciphertext. In
//! an encrypted vault each target is instead materialized as a decrypted copy
//! in the runtime directory, mode `0400`, and the editor is sent there.
//!
//! Read-only on purpose: nothing writes these copies back, so editing a note is
//! still `ntropy search`. A `0400` file makes an editor complain at save time
//! rather than silently discard the work. Write-back is deferred; see
//! `todos/01kz9zn4gmd3h92rp47k4dw978-lsp-reencrypt-decrypted-target-on-save.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ntropy::id::Id;
use ntropy::session::VaultSession;

/// The decrypted copies this server session has handed out.
///
/// One per note identity, reused across jumps so following the same link twice
/// does not litter the runtime directory, and all of them removed when the
/// server's temporary directory drops at shutdown.
#[derive(Debug)]
pub struct DecryptedTargets {
    dir: tempfile::TempDir,
    by_id: HashMap<Id, PathBuf>,
}

impl DecryptedTargets {
    /// Create the session's staging area.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            dir: tempfile::Builder::new()
                .prefix("ntropy-lsp-")
                .tempdir_in(crate::run::securetemp::runtime_dir())?,
            by_id: HashMap::new(),
        })
    }

    /// A readable path for the note at `path`, decrypting it if needed.
    ///
    /// Returns `path` unchanged when the vault is not encrypted: there is
    /// nothing to decrypt and the editor should see the real note.
    pub fn readable_path(&mut self, session: &VaultSession, id: Id, path: &Path) -> PathBuf {
        if !session.is_encrypted() {
            return path.to_path_buf();
        }
        if let Some(existing) = self.by_id.get(&id)
            && existing.is_file()
        {
            return existing.clone();
        }
        match self.materialize(session, id, path) {
            Some(copy) => copy,
            // Falling back to the ciphertext would open binary in the editor.
            // Returning the original at least keeps the failure legible, and
            // it is the same path every other surface reports.
            None => path.to_path_buf(),
        }
    }

    /// Write a `0400` decrypted copy and remember it.
    fn materialize(&mut self, session: &VaultSession, id: Id, path: &Path) -> Option<PathBuf> {
        use std::os::unix::fs::PermissionsExt;

        let content = session.cipher().read(path).ok()?;
        // Named after the link target so the editor shows a title, not a bare
        // ULID, and treats the buffer as Markdown.
        let name = format!("{id}.md");
        let copy = self.dir.path().join(&name);

        std::fs::write(&copy, content).ok()?;
        std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o400)).ok()?;

        self.by_id.insert(id, copy.clone());
        Some(copy)
    }
}

#[cfg(all(test, feature = "encryption"))]
mod tests {
    use super::*;
    use ntropy::cipher::AgeCipher;
    use ntropy::crypto::age_io;
    use ntropy::vault::Vault;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn encrypted_vault() -> (tempfile::TempDir, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        let (identity, recipient) = age_io::generate_keypair();
        std::fs::write(
            root.join(".ntropy/identity.pub"),
            format!("{}\n", recipient.as_str()),
        )
        .expect("recipient");
        let session = VaultSession::with_cipher(
            Vault::new(root),
            Arc::new(AgeCipher::new(recipient, Some(identity))),
        );
        (dir, session)
    }

    fn write_note(session: &VaultSession, content: &str) -> PathBuf {
        let path = session.layout().all_notes().join(format!("{ULID}.age"));
        session.cipher().write(&path, content).expect("write");
        path
    }

    fn id() -> Id {
        ULID.parse().expect("ulid")
    }

    #[test]
    fn a_plaintext_vault_gets_its_real_note_back() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));
        let note = dir.path().join("all-notes").join("note.md");

        let mut targets = DecryptedTargets::new().expect("staging");
        assert_eq!(targets.readable_path(&session, id(), &note), note);
    }

    #[test]
    fn an_encrypted_note_is_materialized_in_the_clear() {
        let (_guard, session) = encrypted_vault();
        let note = write_note(&session, "---\ntitle: A\n---\nbody\n");

        let mut targets = DecryptedTargets::new().expect("staging");
        let readable = targets.readable_path(&session, id(), &note);

        assert_ne!(readable, note);
        assert_eq!(
            std::fs::read_to_string(&readable).expect("read"),
            "---\ntitle: A\n---\nbody\n"
        );
    }

    #[test]
    fn the_copy_lives_outside_the_vault() {
        // Inside it, a sync provider would upload the plaintext.
        let (_guard, session) = encrypted_vault();
        let note = write_note(&session, "---\ntitle: A\n---\nbody\n");

        let mut targets = DecryptedTargets::new().expect("staging");
        let readable = targets.readable_path(&session, id(), &note);
        assert!(
            !readable.starts_with(session.root()),
            "{}",
            readable.display()
        );
    }

    #[test]
    fn the_copy_is_read_only() {
        // An editor then refuses the save rather than silently dropping it.
        let (_guard, session) = encrypted_vault();
        let note = write_note(&session, "---\ntitle: A\n---\nbody\n");

        let mut targets = DecryptedTargets::new().expect("staging");
        let readable = targets.readable_path(&session, id(), &note);
        let mode = std::fs::metadata(&readable)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o400, "got {:o}", mode & 0o777);
    }

    #[test]
    fn the_copy_is_markdown_so_editors_highlight_it() {
        let (_guard, session) = encrypted_vault();
        let note = write_note(&session, "---\ntitle: A\n---\nbody\n");

        let mut targets = DecryptedTargets::new().expect("staging");
        let readable = targets.readable_path(&session, id(), &note);
        assert_eq!(readable.extension().and_then(|e| e.to_str()), Some("md"));
    }

    #[test]
    fn following_the_same_link_twice_reuses_one_copy() {
        // Otherwise a long session would fill the runtime directory.
        let (_guard, session) = encrypted_vault();
        let note = write_note(&session, "---\ntitle: A\n---\nbody\n");

        let mut targets = DecryptedTargets::new().expect("staging");
        let first = targets.readable_path(&session, id(), &note);
        let second = targets.readable_path(&session, id(), &note);
        assert_eq!(first, second);
    }

    #[test]
    fn the_copies_are_gone_when_the_session_ends() {
        let (_guard, session) = encrypted_vault();
        let note = write_note(&session, "---\ntitle: A\n---\nbody\n");

        let readable = {
            let mut targets = DecryptedTargets::new().expect("staging");
            targets.readable_path(&session, id(), &note)
        };
        assert!(!readable.exists());
    }

    #[test]
    fn a_note_that_cannot_be_decrypted_falls_back_to_its_own_path() {
        let (_guard, session) = encrypted_vault();
        let note = session.layout().all_notes().join(format!("{ULID}.age"));
        std::fs::write(&note, b"not age ciphertext").expect("write junk");

        let mut targets = DecryptedTargets::new().expect("staging");
        assert_eq!(targets.readable_path(&session, id(), &note), note);
    }
}
