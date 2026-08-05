// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Where an unlocked vault identity is kept between commands.
//!
//! Entries are keyed by the vault's `age1...` recipient rather than by its
//! path, so moving a vault directory does not orphan its stored key. The
//! stored value is the bare `AGE-SECRET-KEY-1...` secret: the passphrase wrap
//! around `.ntropy/identity.age` uses scrypt with a work factor targeting about
//! a second, and paying that on every command would be intolerable.
//!
//! The trait exists so the whole retrieval chain is testable against
//! [`MemoryIdentityStore`]. No test in the standard suite touches a real
//! keychain.

use std::collections::HashMap;
use std::sync::Mutex;

use super::KeyError;

/// A place that holds vault identities between commands.
pub trait IdentityStore: std::fmt::Debug {
    /// The stored secret for `recipient`, or `None` when nothing is stored.
    fn get(&self, recipient: &str) -> Result<Option<String>, KeyError>;

    /// Store `secret` under `recipient`, replacing anything already there.
    fn set(&self, recipient: &str, secret: &str) -> Result<(), KeyError>;

    /// Remove the entry for `recipient`.
    ///
    /// Reports whether an entry was actually removed, so `ntropy lock` can say
    /// whether it did anything.
    fn delete(&self, recipient: &str) -> Result<bool, KeyError>;
}

// =============================================================================
// In-memory
// =============================================================================

/// An identity store held in process memory, for tests.
///
/// Interior mutability keeps the trait's `&self` methods usable without
/// threading a mutable borrow through the retrieval chain.
#[derive(Debug, Default)]
pub struct MemoryIdentityStore {
    entries: Mutex<HashMap<String, String>>,
}

impl MemoryIdentityStore {
    /// An empty store, standing for a machine where nothing is unlocked.
    pub fn new() -> Self {
        Self::default()
    }

    /// A store already holding `secret` for `recipient`, standing for a
    /// machine where `ntropy unlock` has already run.
    pub fn with_entry(recipient: &str, secret: &str) -> Self {
        let store = Self::new();
        store
            .set(recipient, secret)
            .expect("an in-memory store cannot fail to accept an entry");
        store
    }

    /// How many entries are held, for assertions.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether the store holds nothing, for assertions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl IdentityStore for MemoryIdentityStore {
    fn get(&self, recipient: &str) -> Result<Option<String>, KeyError> {
        Ok(self.lock().get(recipient).cloned())
    }

    fn set(&self, recipient: &str, secret: &str) -> Result<(), KeyError> {
        self.lock().insert(recipient.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, recipient: &str) -> Result<bool, KeyError> {
        Ok(self.lock().remove(recipient).is_some())
    }
}

// =============================================================================
// Never stores anything
// =============================================================================

/// A store that holds nothing and accepts nothing.
///
/// Used where an identity must not outlive the command that obtained it: a
/// one-off `--identity` run has no reason to populate the keychain, and
/// nothing should be written to a user's keychain as a side effect of a
/// command they did not ask to unlock anything.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullIdentityStore;

impl IdentityStore for NullIdentityStore {
    fn get(&self, _recipient: &str) -> Result<Option<String>, KeyError> {
        Ok(None)
    }

    fn set(&self, _recipient: &str, _secret: &str) -> Result<(), KeyError> {
        Ok(())
    }

    fn delete(&self, _recipient: &str) -> Result<bool, KeyError> {
        Ok(false)
    }
}

// =============================================================================
// The OS credential store
// =============================================================================

#[cfg(feature = "encryption")]
mod os {
    use std::sync::Arc;

    use keyring_core::{CredentialStore, Entry};

    use super::{IdentityStore, KeyError};

    /// The service name every ntropy entry is filed under.
    pub const KEYRING_SERVICE: &str = "ntropy";

    /// The platform credential store: macOS Keychain, or on other Unixes the
    /// Secret Service with the kernel keyring as a fallback.
    pub struct KeyringStore {
        store: Arc<CredentialStore>,
    }

    impl KeyringStore {
        /// Open the platform store.
        ///
        /// Fails when no store is available at all, which on a headless Linux
        /// box without a Secret Service daemon or kernel keyring support is a
        /// real outcome, not an error to paper over: the caller falls back to
        /// asking for the passphrase.
        pub fn open() -> Result<Self, KeyError> {
            Ok(Self {
                store: platform_store()?,
            })
        }

        fn entry(&self, recipient: &str) -> Result<Entry, KeyError> {
            self.store
                .build(KEYRING_SERVICE, recipient, None)
                .map_err(KeyError::Keyring)
        }
    }

    impl std::fmt::Debug for KeyringStore {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "KeyringStore({})", self.store.vendor())
        }
    }

    impl IdentityStore for KeyringStore {
        fn get(&self, recipient: &str) -> Result<Option<String>, KeyError> {
            match self.entry(recipient)?.get_password() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(e) => Err(KeyError::Keyring(e)),
            }
        }

        fn set(&self, recipient: &str, secret: &str) -> Result<(), KeyError> {
            self.entry(recipient)?
                .set_password(secret)
                .map_err(KeyError::Keyring)
        }

        fn delete(&self, recipient: &str) -> Result<bool, KeyError> {
            match self.entry(recipient)?.delete_credential() {
                Ok(()) => Ok(true),
                Err(keyring_core::Error::NoEntry) => Ok(false),
                Err(e) => Err(KeyError::Keyring(e)),
            }
        }
    }

    /// The macOS Keychain.
    ///
    /// Keychain ACLs bind to the code signature, so an unsigned binary (one
    /// built by `cargo install`, for instance) prompts for authorization the
    /// first time it reads an entry it did not create.
    #[cfg(target_os = "macos")]
    fn platform_store() -> Result<Arc<CredentialStore>, KeyError> {
        let store =
            apple_native_keyring_store::keychain::Store::new().map_err(KeyError::Keyring)?;
        Ok(store as Arc<CredentialStore>)
    }

    /// The Secret Service, falling back to the kernel keyring.
    ///
    /// Secret Service needs a running daemon and a session bus, which a
    /// headless box or a bare SSH session generally lacks. The kernel keyring
    /// needs neither, so it is what makes an unlocked session possible there.
    /// Its entries do not survive a reboot, which is the trade for needing no
    /// daemon.
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    fn platform_store() -> Result<Arc<CredentialStore>, KeyError> {
        match zbus_secret_service_keyring_store::Store::new() {
            Ok(store) => Ok(store as Arc<CredentialStore>),
            // The Secret Service failure is the one worth reporting: the
            // kernel keyring is the fallback, so its absence is expected on a
            // desktop and says nothing useful about what went wrong.
            Err(secret_service_error) => match linux_keyutils_keyring_store::Store::new() {
                Ok(store) => Ok(store as Arc<CredentialStore>),
                Err(_) => Err(KeyError::Keyring(secret_service_error)),
            },
        }
    }

    /// No credential store is known for this platform.
    #[cfg(not(any(
        target_os = "macos",
        all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        )
    )))]
    fn platform_store() -> Result<Arc<CredentialStore>, KeyError> {
        Err(KeyError::NoCredentialStore)
    }
}

#[cfg(feature = "encryption")]
pub use os::{KEYRING_SERVICE, KeyringStore};

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPIENT: &str = "age1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsxxxxxx";
    const OTHER: &str = "age1zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzsyyyyyy";
    const SECRET: &str = "AGE-SECRET-KEY-1TESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTTESTT";

    #[test]
    fn a_fresh_memory_store_holds_nothing() {
        let store = MemoryIdentityStore::new();
        assert!(store.is_empty());
        assert_eq!(store.get(RECIPIENT).expect("get"), None);
    }

    #[test]
    fn memory_store_roundtrips_a_secret() {
        let store = MemoryIdentityStore::new();
        store.set(RECIPIENT, SECRET).expect("set");
        assert_eq!(store.get(RECIPIENT).expect("get").as_deref(), Some(SECRET));
    }

    #[test]
    fn memory_store_keys_entries_separately_per_recipient() {
        // The keying decision in miniature: two vaults on one machine must not
        // see each other's keys.
        let store = MemoryIdentityStore::new();
        store.set(RECIPIENT, SECRET).expect("set");
        assert_eq!(store.get(OTHER).expect("get"), None);
    }

    #[test]
    fn setting_twice_replaces_the_entry() {
        let store = MemoryIdentityStore::new();
        store.set(RECIPIENT, SECRET).expect("set");
        store
            .set(RECIPIENT, "AGE-SECRET-KEY-1REPLACED")
            .expect("set");
        assert_eq!(
            store.get(RECIPIENT).expect("get").as_deref(),
            Some("AGE-SECRET-KEY-1REPLACED")
        );
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn delete_reports_whether_it_removed_anything() {
        // `ntropy lock` reports "was unlocked" versus "was already locked"
        // from this boolean, so it is behaviour and not a convenience.
        let store = MemoryIdentityStore::with_entry(RECIPIENT, SECRET);
        assert!(store.delete(RECIPIENT).expect("delete"));
        assert!(!store.delete(RECIPIENT).expect("delete again"));
        assert!(store.is_empty());
    }

    #[test]
    fn with_entry_starts_out_populated() {
        let store = MemoryIdentityStore::with_entry(RECIPIENT, SECRET);
        assert_eq!(store.get(RECIPIENT).expect("get").as_deref(), Some(SECRET));
    }

    #[test]
    fn the_null_store_forgets_everything_it_is_given() {
        // What makes a one-off `--identity` run leave no trace behind.
        let store = NullIdentityStore;
        store.set(RECIPIENT, SECRET).expect("set");
        assert_eq!(store.get(RECIPIENT).expect("get"), None);
        assert!(!store.delete(RECIPIENT).expect("delete"));
    }

    /// Exercises the real platform credential store.
    ///
    /// Ignored by default: it writes to the developer's login keychain, and on
    /// macOS an unsigned test binary triggers an authorization dialog. Run it
    /// by hand once per platform with
    /// `cargo test --lib keys::store::tests::real -- --ignored --nocapture`.
    #[test]
    #[ignore = "touches the real OS credential store"]
    #[cfg(feature = "encryption")]
    fn real_keyring_store_roundtrips_and_deletes() {
        let store = KeyringStore::open().expect("open the platform credential store");
        let recipient = "age1ntropy-selftest-please-delete-me";

        store.set(recipient, SECRET).expect("set");
        assert_eq!(store.get(recipient).expect("get").as_deref(), Some(SECRET));
        assert!(store.delete(recipient).expect("delete"));
        assert_eq!(store.get(recipient).expect("get"), None);
        assert!(!store.delete(recipient).expect("delete again"));
    }
}
