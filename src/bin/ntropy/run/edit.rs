// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The edit round trip (ADR 0041).
//!
//! A plaintext vault hands the editor the note itself, exactly as ntropy always
//! has. An encrypted vault cannot: the file on disk is ciphertext, so the note
//! is decrypted to a `0600` file outside the vault, edited there, and encrypted
//! back when the editor exits.
//!
//! The launcher is injected rather than called directly, which is what makes
//! any of this testable. Following the picker's precedent (ADR 0021), the logic
//! lives here and is unit-tested, while the terminal-bound spawn stays in
//! [`super::editor`] and is validated by hand.

use std::path::Path;

use anyhow::{Context, Result, bail};

use ntropy::session::VaultSession;

use super::securetemp::SecureTempFile;

/// Opens a file for the user and returns once they are done with it.
///
/// The real implementation spawns `$VISUAL`/`$EDITOR`; tests pass a closure
/// that manipulates the file directly.
pub trait Launcher {
    fn launch(&self, path: &Path) -> Result<()>;
}

impl<F> Launcher for F
where
    F: Fn(&Path) -> Result<()>,
{
    fn launch(&self, path: &Path) -> Result<()> {
        self(path)
    }
}

/// What an edit did.
#[derive(Debug, PartialEq, Eq)]
pub struct EditOutcome {
    /// Whether the note's stored bytes changed.
    pub wrote: bool,
}

/// Edit the note at `note_path`, decrypting first when the vault requires it.
///
/// `initial` is the content the caller already has in hand. `new` and `today`
/// pass what they just wrote, so the round trip does not decrypt a note it
/// rendered moments earlier — and, on a locked vault, does not need the key to
/// read back something it only just encrypted.
pub fn edit_note(
    session: &VaultSession,
    note_path: &Path,
    initial: Option<&str>,
    launcher: &dyn Launcher,
) -> Result<EditOutcome> {
    if !session.is_encrypted() {
        // A plaintext vault edits the file in place, as it always has. Sending
        // it through a temp file would relocate vim's swap and undo files, hide
        // the note's real path from editor plugins, and change what realignment
        // sees — none of which encryption asks for.
        launcher.launch(note_path)?;
        return Ok(EditOutcome { wrote: true });
    }

    let name = note_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("note");
    // `.md` so the editor treats it as Markdown; the on-disk name ends in
    // `.age`, which no editor knows anything about.
    let temp_name = format!("{}.md", name.trim_end_matches(".age"));
    let temp = SecureTempFile::create(&temp_name)?;

    // What the note looked like going in, so an edit that changed nothing can
    // be recognized, and so a concurrent write can be detected.
    let before = match initial {
        Some(content) => content.to_owned(),
        None => session
            .cipher()
            .read(note_path)
            .with_context(|| format!("while decrypting `{}`", note_path.display()))?,
    };
    let guard = Fingerprint::of(note_path);
    temp.write(&before)?;

    // A non-zero exit is still reported, but not before the work is saved: in a
    // plaintext vault `:cq` leaves whatever was written on disk, and the
    // encrypted path keeps that promise rather than discarding the buffer.
    let launch_result = launcher.launch(temp.path());

    let after = temp.read()?;
    if after == before {
        launch_result?;
        return Ok(EditOutcome { wrote: false });
    }

    if Fingerprint::of(note_path) != guard {
        // Something else wrote the note while the editor was open. Writing now
        // would silently discard whichever change lost the race, so neither is
        // taken and the user is pointed at their own version.
        let kept = temp.keep();
        bail!(
            "`{}` changed while you were editing it; your version is at `{}`",
            note_path.display(),
            kept.display()
        );
    }

    session
        .cipher()
        .write(note_path, &after)
        .with_context(|| format!("while encrypting `{}`", note_path.display()))?;

    launch_result?;
    Ok(EditOutcome { wrote: true })
}

/// Enough of a file's identity to notice someone else rewriting it.
///
/// Modification time and length rather than a hash of the ciphertext: the
/// comparison happens on every edit, and re-reading a whole note to detect an
/// event this rare is not a trade worth making. A `None` means the file was
/// absent, which is how a newly created note starts out.
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint(Option<(std::time::SystemTime, u64)>);

impl Fingerprint {
    fn of(path: &Path) -> Self {
        Self(
            std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok().map(|t| (t, m.len()))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntropy::vault::Vault;
    use std::path::PathBuf;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// A launcher that appends a line, as an editor session would.
    #[cfg(feature = "encryption")]
    fn append(line: &'static str) -> impl Fn(&Path) -> Result<()> {
        move |path: &Path| {
            let mut content = std::fs::read_to_string(path)?;
            content.push_str(line);
            std::fs::write(path, content)?;
            Ok(())
        }
    }

    /// A launcher that leaves the file alone.
    #[cfg(feature = "encryption")]
    fn no_op(_path: &Path) -> Result<()> {
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Plaintext vaults
    // -------------------------------------------------------------------------

    #[test]
    fn a_plaintext_vault_edits_the_note_in_place() {
        // The editor must see the note's real path, not a temp copy.
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));

        let note = dir.path().join("all-notes").join(format!("{ULID}-a.md"));
        std::fs::write(&note, "---\ntitle: A\n---\nbody\n").expect("write");

        let seen = std::cell::RefCell::new(PathBuf::new());
        edit_note(&session, &note, None, &|path: &Path| {
            *seen.borrow_mut() = path.to_path_buf();
            Ok(())
        })
        .expect("edit");

        assert_eq!(*seen.borrow(), note);
    }

    // -------------------------------------------------------------------------
    // Encrypted vaults
    // -------------------------------------------------------------------------

    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use ntropy::cipher::AgeCipher;
        use ntropy::crypto::{Identity, age_io};
        use std::sync::Arc;

        /// An encrypted vault and the key that opens it.
        ///
        /// The library's own fixtures are `#[cfg(test)]` and so out of reach from
        /// the binary crate; this is the same scaffold in miniature.
        struct Fixture {
            _dir: tempfile::TempDir,
            session: VaultSession,
            identity: Identity,
        }

        impl Fixture {
            /// A session over the same vault with no identity.
            fn locked(&self) -> VaultSession {
                VaultSession::with_cipher(
                    Vault::new(self.session.root()),
                    Arc::new(AgeCipher::new(self.identity.to_public(), None)),
                )
            }
        }

        fn encrypted_vault() -> Fixture {
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
                Arc::new(AgeCipher::new(recipient, Some(identity.clone()))),
            );
            Fixture {
                _dir: dir,
                session,
                identity,
            }
        }

        fn write_encrypted_note(session: &VaultSession, ulid: &str, content: &str) -> PathBuf {
            let path = session.layout().all_notes().join(format!("{ulid}.age"));
            session.cipher().write(&path, content).expect("write note");
            path
        }

        #[test]
        fn the_editor_never_sees_the_ciphertext() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");

            let seen = std::cell::RefCell::new(PathBuf::new());
            edit_note(session, &note, None, &|path: &Path| {
                *seen.borrow_mut() = path.to_path_buf();
                // What the editor opens is the note, in the clear.
                assert_eq!(
                    std::fs::read_to_string(path).expect("read"),
                    "---\ntitle: A\n---\nbody\n"
                );
                Ok(())
            })
            .expect("edit");

            let seen = seen.borrow();
            assert_ne!(*seen, note);
            assert!(
                !seen.starts_with(session.root()),
                "the temp file must live outside the vault: {}",
                seen.display()
            );
            assert_eq!(seen.extension().and_then(|e| e.to_str()), Some("md"));
        }

        #[test]
        fn a_changed_note_is_encrypted_back() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");

            let outcome = edit_note(session, &note, None, &append("appended\n")).expect("edit");
            assert!(outcome.wrote);
            assert_eq!(
                session.cipher().read(&note).expect("read"),
                "---\ntitle: A\n---\nbody\nappended\n"
            );
        }

        #[test]
        fn an_unchanged_note_is_not_rewritten() {
            // Re-encrypting produces fresh ciphertext every time, so writing
            // an unchanged note would hand a sync provider a "change" to
            // upload for an edit that did nothing.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");

            let before = std::fs::read(&note).expect("read");
            let mtime = std::fs::metadata(&note).expect("meta").modified().ok();

            let outcome = edit_note(session, &note, None, &no_op).expect("edit");
            assert!(!outcome.wrote);
            assert_eq!(std::fs::read(&note).expect("read"), before);
            assert_eq!(
                std::fs::metadata(&note).expect("meta").modified().ok(),
                mtime
            );
        }

        #[test]
        fn the_temp_file_is_gone_afterwards() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");

            let seen = std::cell::RefCell::new(PathBuf::new());
            edit_note(session, &note, None, &|path: &Path| {
                *seen.borrow_mut() = path.to_path_buf();
                Ok(())
            })
            .expect("edit");
            assert!(!seen.borrow().exists());
        }

        #[test]
        fn a_non_zero_exit_still_saves_the_work_then_reports() {
            // Both halves of the promise: in a plaintext vault `:cq` keeps
            // whatever was written, and the error is still surfaced.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");

            let err = edit_note(session, &note, None, &|path: &Path| {
                let mut content = std::fs::read_to_string(path)?;
                content.push_str("saved before quitting\n");
                std::fs::write(path, content)?;
                bail!("editor exited with a non-zero status")
            })
            .expect_err("the editor failed");
            assert!(err.to_string().contains("non-zero"), "{err}");

            assert_eq!(
                session.cipher().read(&note).expect("read"),
                "---\ntitle: A\n---\nbody\nsaved before quitting\n",
                "the saved buffer must reach the note"
            );
        }

        #[test]
        fn a_failed_launch_that_wrote_nothing_leaves_the_note_alone() {
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");
            let before = std::fs::read(&note).expect("read");

            edit_note(session, &note, None, &|_: &Path| {
                bail!("editor could not be spawned")
            })
            .expect_err("spawn failed");
            assert_eq!(std::fs::read(&note).expect("read"), before);
        }

        #[test]
        fn a_note_changed_underneath_is_refused_and_the_work_kept() {
            // Last-writer-wins would silently discard one of the two edits.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let note = write_encrypted_note(session, ULID, "---\ntitle: A\n---\nbody\n");

            let err = edit_note(session, &note, None, &|path: &Path| {
                let mut content = std::fs::read_to_string(path)?;
                content.push_str("my edit\n");
                std::fs::write(path, content)?;

                // Another process rewrites the note while the editor is open.
                std::thread::sleep(std::time::Duration::from_millis(10));
                session
                    .cipher()
                    .write(&path_of_note(session), "---\ntitle: A\n---\nsomeone else\n")
                    .expect("concurrent write");
                Ok(())
            })
            .expect_err("refused");

            let message = err.to_string();
            assert!(
                message.contains("changed while you were editing"),
                "{message}"
            );

            // The other writer's version survives untouched.
            assert_eq!(
                session.cipher().read(&note).expect("read"),
                "---\ntitle: A\n---\nsomeone else\n"
            );

            // And the user's version is recoverable at the named path.
            let kept = message
                .rsplit('`')
                .nth(1)
                .expect("the error names the kept file");
            assert!(
                std::fs::read_to_string(kept)
                    .expect("read kept")
                    .contains("my edit"),
                "the user's work must survive"
            );
            let _ = std::fs::remove_file(kept);
        }

        /// The note path inside the closure above.
        fn path_of_note(session: &VaultSession) -> PathBuf {
            session.layout().all_notes().join(format!("{ULID}.age"))
        }

        #[test]
        fn supplied_content_means_the_note_is_never_read() {
            // How `new` works on a locked vault: it wrote the note itself, so
            // handing that content back avoids a decrypt it has no key for.
            let fixture = encrypted_vault();
            let locked = fixture.locked();
            let template = "---\ntitle: Fresh\n---\n";
            // `create_note` has already written this, encrypting with the
            // public recipient alone.
            let note = write_encrypted_note(&fixture.session, ULID, template);

            assert!(!locked.is_unlocked());
            let outcome = edit_note(
                &locked,
                &note,
                Some(template),
                &append("written while locked\n"),
            )
            .expect("edit");

            assert!(outcome.wrote);
            assert_eq!(
                fixture.session.cipher().read(&note).expect("read"),
                "---\ntitle: Fresh\n---\nwritten while locked\n"
            );
        }

        #[test]
        fn an_abandoned_creation_leaves_the_note_as_created() {
            // `create_note` already wrote it, so an editor that changed
            // nothing must simply not rewrite it.
            let fixture = encrypted_vault();
            let session = &fixture.session;
            let template = "---\ntitle: Fresh\n---\n";
            let note = write_encrypted_note(session, ULID, template);
            let before = std::fs::read(&note).expect("read");

            let outcome = edit_note(session, &note, Some(template), &no_op).expect("edit");
            assert!(!outcome.wrote);
            assert_eq!(std::fs::read(&note).expect("read"), before);
        }
    }
}
