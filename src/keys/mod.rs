// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Getting hold of a vault's identity (ADR 0041).
//!
//! Shaped after [`crate::config::global`]: every decision lives in a function
//! taking what it needs explicitly, so the whole chain is testable against a
//! temp directory and an in-memory store, while the thin no-argument wrappers
//! resolve the real OS credential store.
//!
//! The chain, in order, is: an identity file named by `--identity` or
//! `NTROPY_IDENTITY`; the OS credential store; a passphrase file; an
//! interactive prompt. Running out of options is not an error but a *locked*
//! vault, which callers report differently depending on whether they needed to
//! read anything.
//!
//! Only the first step yields an identity without a passphrase, and only the
//! last two can produce one where nothing was stored before. A passphrase
//! obtained from either is written to the store on the way out, so an ordinary
//! command that had to ask leaves the vault unlocked exactly as `ntropy
//! unlock` would have.

pub mod prompt;
pub mod store;

use std::path::{Path, PathBuf};

use crate::cipher::Passphrase;
use crate::fsutil;

pub use prompt::PassphrasePrompt;
pub use store::IdentityStore;

#[cfg(feature = "encryption")]
use crate::crypto::{CryptoError, Identity, Recipient, age_io};

/// Why an identity could not be obtained.
#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    /// A file could not be read or written.
    #[error(transparent)]
    Fs(#[from] fsutil::FsError),

    /// The identity file named by `--identity` or `NTROPY_IDENTITY` is absent.
    #[error("no identity file at `{}`", .0.display())]
    IdentityFileMissing(PathBuf),

    /// The identity file holds no usable age identity.
    #[error("`{}` does not contain an age identity", .0.display())]
    IdentityFileMalformed(PathBuf),

    /// The supplied identity belongs to a different vault.
    ///
    /// Loud on purpose: silently accepting it would leave every note in the
    /// vault unreadable with no explanation of why.
    #[error(
        "the identity in `{}` belongs to a different vault \
         (it is for `{found}`, this vault uses `{expected}`)",
        path.display()
    )]
    RecipientMismatch {
        /// Where the wrong identity came from.
        path: PathBuf,
        /// The recipient the supplied identity actually belongs to.
        found: String,
        /// The recipient this vault expects.
        expected: String,
    },

    /// The passphrase file is absent.
    #[error("no passphrase file at `{}`", .0.display())]
    PassphraseFileMissing(PathBuf),

    /// The passphrase file holds nothing.
    #[error("the passphrase file `{}` is empty", .0.display())]
    PassphraseFileEmpty(PathBuf),

    /// `.ntropy/identity.pub` does not hold a valid recipient.
    #[error("`{}` does not contain a valid age recipient", .0.display())]
    MalformedRecipientFile(PathBuf),

    /// The credential store rejected an operation.
    #[cfg(feature = "encryption")]
    #[error("while using the OS credential store")]
    Keyring(#[source] keyring_core::Error),

    /// No credential store exists on this platform.
    #[cfg(feature = "encryption")]
    #[error("no OS credential store is available on this platform")]
    NoCredentialStore,

    /// Nothing could supply an identity and there was no way to ask for one.
    #[error("this vault is locked; run `ntropy unlock`")]
    Locked,

    /// A cryptographic step failed, most often a wrong passphrase.
    #[cfg(feature = "encryption")]
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

// =============================================================================
// The passphrase file
// =============================================================================

/// Read a passphrase from the first line of `path`.
///
/// Only the first line, with its trailing newline removed, because a
/// passphrase file is something a script or a password manager writes and
/// those overwhelmingly end with a newline. Nothing else is trimmed: leading
/// and interior whitespace are part of the passphrase.
///
/// An empty file is an error rather than an empty passphrase, which the wrap
/// would refuse anyway; catching it here names the file.
pub fn read_passphrase_file(path: &Path) -> Result<Passphrase, KeyError> {
    let contents = fsutil::read_to_string_if_exists(path)?
        .ok_or_else(|| KeyError::PassphraseFileMissing(path.to_path_buf()))?;

    let first_line = contents
        .split_inclusive('\n')
        .next()
        .unwrap_or("")
        .strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .unwrap_or_else(|| contents.as_str());

    if first_line.is_empty() {
        return Err(KeyError::PassphraseFileEmpty(path.to_path_buf()));
    }
    Ok(Passphrase::new(first_line))
}

// =============================================================================
// Everything below needs the crypto layer
// =============================================================================

#[cfg(feature = "encryption")]
mod acquire {
    use super::*;

    /// Where the vault's public recipient lives, and everything needed to find
    /// its identity.
    ///
    /// A borrowed bundle rather than a long argument list, so the retrieval
    /// order lives in one place and every caller supplies the same shape.
    pub struct Acquisition<'a> {
        /// The vault's recipient, from `.ntropy/identity.pub`.
        pub recipient: &'a Recipient,
        /// Path to the scrypt-wrapped identity, `.ntropy/identity.age`.
        pub wrapped_identity: &'a Path,
        /// An identity file named by `--identity` or `NTROPY_IDENTITY`.
        pub identity_file: Option<&'a Path>,
        /// A passphrase file named by `--passphrase-file`.
        pub passphrase_file: Option<&'a Path>,
        /// Where an unlocked identity is kept between commands.
        pub store: &'a dyn IdentityStore,
        /// How to ask a human, when there is a human to ask.
        pub prompt: Option<&'a dyn PassphrasePrompt>,
    }

    impl std::fmt::Debug for Acquisition<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Acquisition")
                .field("recipient", &self.recipient)
                .field("wrapped_identity", &self.wrapped_identity)
                .field("identity_file", &self.identity_file)
                .field("passphrase_file", &self.passphrase_file)
                .field("has_prompt", &self.prompt.is_some())
                .finish()
        }
    }

    /// Obtain the vault's identity, or report that the vault is locked.
    ///
    /// `Ok(None)` means locked: nothing was available and there was no way to
    /// ask. That is a normal outcome for a headless command and for the
    /// language server, which never prompts.
    pub fn acquire(request: &Acquisition<'_>) -> Result<Option<Identity>, KeyError> {
        // An explicitly named identity wins outright. It is the escape hatch
        // for scripts and for externally managed keys, and consulting the
        // credential store first would let a stale stored entry silently
        // override what the user just pointed at.
        if let Some(path) = request.identity_file {
            let identity = read_identity_file(path)?;
            let found = identity.to_public();
            if found != *request.recipient {
                return Err(KeyError::RecipientMismatch {
                    path: path.to_path_buf(),
                    found: found.as_str(),
                    expected: request.recipient.as_str(),
                });
            }
            return Ok(Some(identity));
        }

        let recipient = request.recipient.as_str();
        if let Some(secret) = request.store.get(&recipient)? {
            // A stored secret that no longer parses, or that belongs to some
            // other key, is stale rather than fatal: fall through and unwrap
            // the identity file, then overwrite the bad entry.
            if let Ok(identity) = secret.parse::<Identity>()
                && identity.to_public() == *request.recipient
            {
                return Ok(Some(identity));
            }
        }

        // Both remaining sources produce a passphrase, but only one of them
        // should leave the vault unlocked afterwards.
        let (passphrase, asked_a_human) = match (request.passphrase_file, request.prompt) {
            (Some(path), _) => (read_passphrase_file(path)?, false),
            (None, Some(prompt)) => (prompt.existing(&recipient)?, true),
            (None, None) => return Ok(None),
        };

        let identity = unwrap_identity_at(request.wrapped_identity, &passphrase)?;

        // Storing is what makes the explicit `unlock` command a formality:
        // someone who just typed their passphrase should not be asked again.
        //
        // A run that named a passphrase file gets no such treatment. It
        // already has a non-interactive way in and did not ask to unlock this
        // machine, so quietly writing a key into the user's credential store
        // would be a side effect nobody requested — and in the test suite, one
        // that pollutes the developer's real keychain.
        if asked_a_human {
            request.store.set(&recipient, &identity.expose_secret())?;
        }
        Ok(Some(identity))
    }

    /// Obtain the identity, refusing to report a locked vault.
    ///
    /// What `ntropy unlock` runs: there is no useful "locked" answer, because
    /// being locked is precisely what the command exists to fix.
    pub fn unlock(request: &Acquisition<'_>) -> Result<Identity, KeyError> {
        acquire(request)?.ok_or(KeyError::Locked)
    }

    /// Forget the stored identity for `recipient`.
    ///
    /// Reports whether anything was actually forgotten.
    pub fn lock(store: &dyn IdentityStore, recipient: &Recipient) -> Result<bool, KeyError> {
        store.delete(&recipient.as_str())
    }

    // -------------------------------------------------------------------------
    // The files a vault keeps
    // -------------------------------------------------------------------------

    /// Read the vault's recipient from `path`, treating absence as "this vault
    /// is not encrypted".
    pub fn read_recipient_at(path: &Path) -> Result<Option<Recipient>, KeyError> {
        let Some(contents) = fsutil::read_to_string_if_exists(path)? else {
            return Ok(None);
        };
        contents
            .parse::<Recipient>()
            .map(Some)
            .map_err(|_| KeyError::MalformedRecipientFile(path.to_path_buf()))
    }

    /// Read an age identity from a file in `age-keygen` format.
    ///
    /// `age-keygen` writes `# created:` and `# public key:` comment lines
    /// above the secret, and the design promises externally managed identities
    /// work unmodified, so comments and blank lines are skipped rather than
    /// requiring the user to strip them.
    pub fn read_identity_file(path: &Path) -> Result<Identity, KeyError> {
        let contents = fsutil::read_to_string_if_exists(path)?
            .ok_or_else(|| KeyError::IdentityFileMissing(path.to_path_buf()))?;

        contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .find_map(|line| line.parse::<Identity>().ok())
            .ok_or_else(|| KeyError::IdentityFileMalformed(path.to_path_buf()))
    }

    /// Write a vault's keypair: the recipient in the clear, the identity
    /// wrapped under `passphrase`.
    ///
    /// The wrapped file is created with `0600`. The recipient file is not
    /// secret and keeps default permissions.
    pub fn write_keypair_at(
        recipient_path: &Path,
        wrapped_path: &Path,
        identity: &Identity,
        passphrase: &Passphrase,
    ) -> Result<(), KeyError> {
        // Wrapping first means a rejected passphrase leaves neither file
        // behind, so a failed `init` cannot produce a half-made vault.
        let wrapped = age_io::wrap_identity(identity, passphrase)?;

        fsutil::atomic_write(
            recipient_path,
            format!("{}\n", identity.to_public().as_str()).as_bytes(),
        )?;
        fsutil::atomic_write_private(wrapped_path, wrapped.as_bytes())?;
        Ok(())
    }

    /// Unwrap the identity stored at `path`.
    pub fn unwrap_identity_at(path: &Path, passphrase: &Passphrase) -> Result<Identity, KeyError> {
        let armored = fsutil::read_to_string_if_exists(path)?
            .ok_or_else(|| KeyError::IdentityFileMissing(path.to_path_buf()))?;
        Ok(age_io::unwrap_identity(&armored, passphrase)?)
    }

    /// Remove a vault's key files.
    ///
    /// Used by `vault decrypt` once no note needs them. Missing files are not
    /// an error: the point is that they are gone afterwards.
    pub fn remove_keypair_at(recipient_path: &Path, wrapped_path: &Path) -> Result<(), KeyError> {
        for path in [wrapped_path, recipient_path] {
            if path.exists() {
                fsutil::remove_file(path)?;
            }
        }
        Ok(())
    }

    /// Re-wrap the identity at `path` under a new passphrase.
    ///
    /// Only this one file changes; notes are untouched, because the key inside
    /// the wrapper is the same key.
    pub fn change_passphrase_at(
        path: &Path,
        old: &Passphrase,
        new: &Passphrase,
    ) -> Result<(), KeyError> {
        let identity = unwrap_identity_at(path, old)?;
        let rewrapped = age_io::wrap_identity(&identity, new)?;
        fsutil::atomic_write_private(path, rewrapped.as_bytes())?;
        Ok(())
    }
}

#[cfg(feature = "encryption")]
pub use acquire::{
    Acquisition, acquire, change_passphrase_at, lock, read_identity_file, read_recipient_at,
    remove_keypair_at, unlock, unwrap_identity_at, write_keypair_at,
};

#[cfg(test)]
mod passphrase_file_tests {
    use super::*;

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).expect("write passphrase file");
        path
    }

    #[test]
    fn reads_a_single_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "correct horse");
        assert_eq!(
            read_passphrase_file(&path).expect("read").expose(),
            "correct horse"
        );
    }

    #[test]
    fn strips_the_trailing_newline() {
        // The overwhelmingly common shape, since `echo` and every password
        // manager write one.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "correct horse\n");
        assert_eq!(
            read_passphrase_file(&path).expect("read").expose(),
            "correct horse"
        );
    }

    #[test]
    fn strips_a_carriage_return_before_the_newline() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "correct horse\r\n");
        assert_eq!(
            read_passphrase_file(&path).expect("read").expose(),
            "correct horse"
        );
    }

    #[test]
    fn reads_only_the_first_line() {
        // Everything after the first newline is treated as commentary rather
        // than silently folded into the passphrase.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "the passphrase\nsome notes\nmore\n");
        assert_eq!(
            read_passphrase_file(&path).expect("read").expose(),
            "the passphrase"
        );
    }

    #[test]
    fn keeps_interior_and_leading_whitespace() {
        // Whitespace is part of a passphrase; trimming it would silently
        // change what the user chose.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "  two  words  \n");
        assert_eq!(
            read_passphrase_file(&path).expect("read").expose(),
            "  two  words  "
        );
    }

    #[test]
    fn keeps_a_non_ascii_passphrase_intact() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "übergröße-日本語\n");
        assert_eq!(
            read_passphrase_file(&path).expect("read").expose(),
            "übergröße-日本語"
        );
    }

    #[test]
    fn a_missing_file_names_the_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("no-such-passphrase-file");
        match read_passphrase_file(&path) {
            Err(KeyError::PassphraseFileMissing(reported)) => assert_eq!(reported, path),
            other => panic!("expected PassphraseFileMissing, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_file_is_rejected() {
        // Caught here rather than at the wrap so the error names the file.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "");
        assert!(matches!(
            read_passphrase_file(&path),
            Err(KeyError::PassphraseFileEmpty(_))
        ));
    }

    #[test]
    fn a_file_holding_only_a_newline_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write(dir.path(), "pw", "\n");
        assert!(matches!(
            read_passphrase_file(&path),
            Err(KeyError::PassphraseFileEmpty(_))
        ));
    }
}

#[cfg(all(test, feature = "encryption"))]
mod tests {
    use super::store::{MemoryIdentityStore, NullIdentityStore};
    use super::*;
    use crate::crypto::age_io;
    use prompt::FixedPassphrase;

    /// A vault's key material on disk, plus somewhere to put test files.
    struct Fixture {
        dir: tempfile::TempDir,
        identity: Identity,
        recipient: Recipient,
        recipient_path: PathBuf,
        wrapped_path: PathBuf,
    }

    impl Fixture {
        fn new(passphrase: &str) -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let recipient_path = dir.path().join("identity.pub");
            let wrapped_path = dir.path().join("identity.age");
            let (identity, recipient) = age_io::generate_keypair();
            write_keypair_at(
                &recipient_path,
                &wrapped_path,
                &identity,
                &Passphrase::new(passphrase),
            )
            .expect("write keypair");
            Self {
                dir,
                identity,
                recipient,
                recipient_path,
                wrapped_path,
            }
        }

        fn recipient_path(&self) -> &Path {
            &self.recipient_path
        }

        fn wrapped_path(&self) -> &Path {
            &self.wrapped_path
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.dir.path().join(name);
            std::fs::write(&path, contents).expect("write file");
            path
        }

        /// A request with everything absent, to be filled in per test.
        fn request<'a>(&'a self, store: &'a dyn IdentityStore) -> Acquisition<'a> {
            Acquisition {
                recipient: &self.recipient,
                wrapped_identity: &self.wrapped_path,
                identity_file: None,
                passphrase_file: None,
                store,
                prompt: None,
            }
        }
    }

    // -------------------------------------------------------------------------
    // Retrieval order
    // -------------------------------------------------------------------------

    #[test]
    fn an_explicit_identity_file_wins_over_the_store() {
        // A stale stored entry must never override what the user just pointed
        // at, so the file is consulted first.
        let fixture = Fixture::new("p");
        let identity_file = fixture.write("id.txt", &fixture.identity.expose_secret());
        let store = MemoryIdentityStore::with_entry(
            &fixture.recipient.as_str(),
            &age_io::generate_keypair().0.expose_secret(),
        );

        let mut request = fixture.request(&store);
        request.identity_file = Some(&identity_file);

        let identity = acquire(&request).expect("acquire").expect("unlocked");
        assert_eq!(identity.to_public(), fixture.recipient);
    }

    #[test]
    fn the_store_is_used_when_no_identity_file_is_given() {
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::with_entry(
            &fixture.recipient.as_str(),
            &fixture.identity.expose_secret(),
        );
        let prompt = FixedPassphrase::new("p");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        let identity = acquire(&request).expect("acquire").expect("unlocked");
        assert_eq!(identity.to_public(), fixture.recipient);
        assert_eq!(prompt.calls(), 0, "a stored key must not trigger a prompt");
    }

    #[test]
    fn the_passphrase_file_is_used_before_the_prompt() {
        let fixture = Fixture::new("from the file");
        let passphrase_file = fixture.write("pw", "from the file\n");
        let store = MemoryIdentityStore::new();
        let prompt = FixedPassphrase::new("from the prompt");

        let mut request = fixture.request(&store);
        request.passphrase_file = Some(&passphrase_file);
        request.prompt = Some(&prompt);

        let identity = acquire(&request).expect("acquire").expect("unlocked");
        assert_eq!(identity.to_public(), fixture.recipient);
        assert_eq!(prompt.calls(), 0, "the file must satisfy the request alone");
    }

    #[test]
    fn the_prompt_is_the_last_resort() {
        let fixture = Fixture::new("asked for");
        let store = MemoryIdentityStore::new();
        let prompt = FixedPassphrase::new("asked for");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        let identity = acquire(&request).expect("acquire").expect("unlocked");
        assert_eq!(identity.to_public(), fixture.recipient);
        assert_eq!(prompt.calls(), 1);
    }

    #[test]
    fn nothing_available_and_nobody_to_ask_is_a_locked_vault() {
        // Not an error: this is the language server's normal state, and the
        // normal state of any headless command on a locked vault.
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::new();
        assert!(
            acquire(&fixture.request(&store))
                .expect("acquire")
                .is_none()
        );
    }

    // -------------------------------------------------------------------------
    // Unlocking as a side effect
    // -------------------------------------------------------------------------

    #[test]
    fn a_prompted_passphrase_leaves_the_vault_unlocked() {
        // What makes the explicit `unlock` command a formality: a command that
        // had to ask stores what it obtained.
        let fixture = Fixture::new("asked for");
        let store = MemoryIdentityStore::new();
        let prompt = FixedPassphrase::new("asked for");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);
        acquire(&request).expect("acquire").expect("unlocked");

        assert_eq!(
            store
                .get(&fixture.recipient.as_str())
                .expect("get")
                .as_deref(),
            Some(fixture.identity.expose_secret().as_str())
        );
    }

    #[test]
    fn a_passphrase_file_does_not_populate_the_store() {
        // A scripted run already has a non-interactive way in and did not ask
        // to unlock this machine. Writing a key into the credential store
        // there would be a side effect nobody requested.
        let fixture = Fixture::new("from the file");
        let passphrase_file = fixture.write("pw", "from the file\n");
        let store = MemoryIdentityStore::new();

        let mut request = fixture.request(&store);
        request.passphrase_file = Some(&passphrase_file);
        acquire(&request).expect("acquire").expect("unlocked");

        assert!(store.is_empty());
    }

    #[test]
    fn an_explicit_identity_file_does_not_populate_the_store() {
        // A one-off `-i` run must not write to the user's keychain as a side
        // effect of a command they did not ask to unlock anything.
        let fixture = Fixture::new("p");
        let identity_file = fixture.write("id.txt", &fixture.identity.expose_secret());
        let store = MemoryIdentityStore::new();

        let mut request = fixture.request(&store);
        request.identity_file = Some(&identity_file);
        acquire(&request).expect("acquire").expect("unlocked");

        assert!(store.is_empty());
    }

    #[test]
    fn a_wrong_passphrase_stores_nothing() {
        let fixture = Fixture::new("right");
        let store = MemoryIdentityStore::new();
        let prompt = FixedPassphrase::new("wrong");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        assert!(matches!(
            acquire(&request),
            Err(KeyError::Crypto(CryptoError::WrongPassphrase))
        ));
        assert!(store.is_empty());
    }

    #[test]
    fn a_null_store_keeps_nothing_it_is_handed() {
        let fixture = Fixture::new("p");
        let store = NullIdentityStore;
        let prompt = FixedPassphrase::new("p");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        assert!(acquire(&request).expect("acquire").is_some());
        assert_eq!(store.get("anything").expect("get"), None);
    }

    // -------------------------------------------------------------------------
    // A stale or wrong stored entry
    // -------------------------------------------------------------------------

    #[test]
    fn a_stored_entry_for_another_key_is_ignored_and_replaced() {
        // A vault that was rekeyed elsewhere, or a recipient reused after a
        // reset: falling through and re-unwrapping beats failing outright.
        let fixture = Fixture::new("p");
        let (other, _) = age_io::generate_keypair();
        let store =
            MemoryIdentityStore::with_entry(&fixture.recipient.as_str(), &other.expose_secret());
        let prompt = FixedPassphrase::new("p");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        let identity = acquire(&request).expect("acquire").expect("unlocked");
        assert_eq!(identity.to_public(), fixture.recipient);
        assert_eq!(
            store
                .get(&fixture.recipient.as_str())
                .expect("get")
                .as_deref(),
            Some(fixture.identity.expose_secret().as_str()),
            "the stale entry must be overwritten"
        );
    }

    #[test]
    fn an_unparseable_stored_entry_is_ignored() {
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::with_entry(&fixture.recipient.as_str(), "garbage");
        let prompt = FixedPassphrase::new("p");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        let identity = acquire(&request).expect("acquire").expect("unlocked");
        assert_eq!(identity.to_public(), fixture.recipient);
    }

    #[test]
    fn an_unparseable_stored_entry_on_a_locked_vault_stays_locked() {
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::with_entry(&fixture.recipient.as_str(), "garbage");
        assert!(
            acquire(&fixture.request(&store))
                .expect("acquire")
                .is_none()
        );
    }

    // -------------------------------------------------------------------------
    // The identity file
    // -------------------------------------------------------------------------

    #[test]
    fn an_identity_for_another_vault_is_refused_loudly() {
        // Accepting it would leave every note unreadable with no explanation.
        let fixture = Fixture::new("p");
        let (other, other_recipient) = age_io::generate_keypair();
        let identity_file = fixture.write("other.txt", &other.expose_secret());
        let store = MemoryIdentityStore::new();

        let mut request = fixture.request(&store);
        request.identity_file = Some(&identity_file);

        match acquire(&request) {
            Err(KeyError::RecipientMismatch {
                path,
                found,
                expected,
            }) => {
                assert_eq!(path, identity_file);
                assert_eq!(found, other_recipient.as_str());
                assert_eq!(expected, fixture.recipient.as_str());
            }
            other => panic!("expected RecipientMismatch, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_identity_file_names_the_path() {
        let fixture = Fixture::new("p");
        let missing = fixture.dir.path().join("no-such-identity");
        let store = MemoryIdentityStore::new();

        let mut request = fixture.request(&store);
        request.identity_file = Some(&missing);

        match acquire(&request) {
            Err(KeyError::IdentityFileMissing(path)) => assert_eq!(path, missing),
            other => panic!("expected IdentityFileMissing, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_identity_file_names_the_path() {
        let fixture = Fixture::new("p");
        let path = fixture.write("junk.txt", "this is not an age identity\n");
        let store = MemoryIdentityStore::new();

        let mut request = fixture.request(&store);
        request.identity_file = Some(&path);

        assert!(matches!(
            acquire(&request),
            Err(KeyError::IdentityFileMalformed(_))
        ));
    }

    #[test]
    fn the_age_keygen_file_format_is_accepted_unmodified() {
        // The design promises externally managed identities work as-is, and
        // `age-keygen` writes two comment lines above the secret.
        let fixture = Fixture::new("p");
        let contents = format!(
            "# created: 2026-08-06T12:00:00+02:00\n# public key: {}\n{}\n",
            fixture.recipient.as_str(),
            fixture.identity.expose_secret()
        );
        let path = fixture.write("age-keygen.txt", &contents);

        let identity = read_identity_file(&path).expect("read");
        assert_eq!(identity.to_public(), fixture.recipient);
    }

    #[test]
    fn an_identity_file_with_blank_lines_is_accepted() {
        let fixture = Fixture::new("p");
        let contents = format!("\n\n{}\n\n", fixture.identity.expose_secret());
        let path = fixture.write("padded.txt", &contents);
        assert_eq!(
            read_identity_file(&path).expect("read").to_public(),
            fixture.recipient
        );
    }

    // -------------------------------------------------------------------------
    // The files a vault keeps
    // -------------------------------------------------------------------------

    #[test]
    fn the_recipient_file_is_written_with_a_trailing_newline() {
        let fixture = Fixture::new("p");
        let contents = std::fs::read_to_string(fixture.recipient_path()).expect("read");
        assert_eq!(contents, format!("{}\n", fixture.recipient.as_str()));
    }

    #[test]
    fn the_wrapped_identity_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new("p");
        let mode = std::fs::metadata(fixture.wrapped_path())
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);
    }

    #[test]
    fn reading_the_recipient_tolerates_the_trailing_newline() {
        let fixture = Fixture::new("p");
        let recipient = read_recipient_at(fixture.recipient_path())
            .expect("read")
            .expect("present");
        assert_eq!(recipient, fixture.recipient);
    }

    #[test]
    fn an_absent_recipient_file_means_the_vault_is_not_encrypted() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(
            read_recipient_at(&dir.path().join("identity.pub"))
                .expect("read")
                .is_none()
        );
    }

    #[test]
    fn a_malformed_recipient_file_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("identity.pub");
        std::fs::write(&path, "not a recipient\n").expect("write");
        assert!(matches!(
            read_recipient_at(&path),
            Err(KeyError::MalformedRecipientFile(_))
        ));
    }

    #[test]
    fn a_rejected_passphrase_writes_no_key_files() {
        // A failed `init` must not leave a half-made vault behind.
        let dir = tempfile::tempdir().expect("temp dir");
        let (identity, _) = age_io::generate_keypair();
        let recipient_path = dir.path().join("identity.pub");
        let wrapped_path = dir.path().join("identity.age");

        assert!(
            write_keypair_at(
                &recipient_path,
                &wrapped_path,
                &identity,
                &Passphrase::new("")
            )
            .is_err()
        );
        assert!(!recipient_path.exists());
        assert!(!wrapped_path.exists());
    }

    // -------------------------------------------------------------------------
    // unlock and lock
    // -------------------------------------------------------------------------

    #[test]
    fn unlock_refuses_to_report_a_locked_vault() {
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::new();
        assert!(matches!(
            unlock(&fixture.request(&store)),
            Err(KeyError::Locked)
        ));
    }

    #[test]
    fn unlock_stores_the_identity() {
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::new();
        let prompt = FixedPassphrase::new("p");

        let mut request = fixture.request(&store);
        request.prompt = Some(&prompt);

        unlock(&request).expect("unlock");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn lock_reports_whether_the_vault_was_unlocked() {
        let fixture = Fixture::new("p");
        let store = MemoryIdentityStore::with_entry(
            &fixture.recipient.as_str(),
            &fixture.identity.expose_secret(),
        );
        assert!(lock(&store, &fixture.recipient).expect("lock"));
        assert!(!lock(&store, &fixture.recipient).expect("lock again"));
    }

    // -------------------------------------------------------------------------
    // Changing the passphrase
    // -------------------------------------------------------------------------

    #[test]
    fn changing_the_passphrase_preserves_the_identity() {
        let fixture = Fixture::new("old");
        change_passphrase_at(
            fixture.wrapped_path(),
            &Passphrase::new("old"),
            &Passphrase::new("new"),
        )
        .expect("change");

        let reopened =
            unwrap_identity_at(fixture.wrapped_path(), &Passphrase::new("new")).expect("unwrap");
        assert_eq!(reopened.to_public(), fixture.recipient);
    }

    #[test]
    fn the_old_passphrase_stops_working() {
        let fixture = Fixture::new("old");
        change_passphrase_at(
            fixture.wrapped_path(),
            &Passphrase::new("old"),
            &Passphrase::new("new"),
        )
        .expect("change");

        assert!(matches!(
            unwrap_identity_at(fixture.wrapped_path(), &Passphrase::new("old")),
            Err(KeyError::Crypto(CryptoError::WrongPassphrase))
        ));
    }

    #[test]
    fn a_wrong_old_passphrase_leaves_the_file_untouched() {
        // Byte-for-byte, because a partially rewritten wrapper would lock the
        // user out of their own vault.
        let fixture = Fixture::new("old");
        let before = std::fs::read(fixture.wrapped_path()).expect("read");

        assert!(
            change_passphrase_at(
                fixture.wrapped_path(),
                &Passphrase::new("wrong"),
                &Passphrase::new("new"),
            )
            .is_err()
        );

        assert_eq!(std::fs::read(fixture.wrapped_path()).expect("read"), before);
    }

    #[test]
    fn the_rewrapped_file_stays_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new("old");
        change_passphrase_at(
            fixture.wrapped_path(),
            &Passphrase::new("old"),
            &Passphrase::new("new"),
        )
        .expect("change");

        let mode = std::fs::metadata(fixture.wrapped_path())
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);
    }

    #[test]
    fn changing_to_an_empty_passphrase_is_refused() {
        let fixture = Fixture::new("old");
        assert!(
            change_passphrase_at(
                fixture.wrapped_path(),
                &Passphrase::new("old"),
                &Passphrase::new(""),
            )
            .is_err()
        );
    }
}
