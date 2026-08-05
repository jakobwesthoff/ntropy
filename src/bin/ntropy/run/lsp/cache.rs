// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The per-session note cache (ADR 0029).
//!
//! Completion, navigation and symbol lookup all read note metadata. Rather than
//! scan the vault on every keystroke, the server keeps an in-memory projection
//! of each vault it has touched, keyed by the canonicalized vault root. It is
//! populated lazily on first use and refreshed by a full rescan when the client
//! reports a file change (or, as a fallback, when a document is opened). The
//! cache is process-local and ephemeral, so it is a session cache rather than a
//! persisted index (it does not contradict ADR 0002).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ntropy::id::Id;
use ntropy::scan;
use ntropy::session::VaultSession;
use ntropy::vault::Vault;

/// A note's cached metadata: everything completion and navigation need without
/// re-reading the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub id: Id,
    pub title: String,
    pub tags: Vec<String>,
    /// Where the note lives on disk: `<ulid>-<slug>.md`, or `<ulid>.age` in an
    /// encrypted vault.
    pub path: PathBuf,
    /// What other notes write when linking to this one.
    ///
    /// Always the `<ulid>-<slug>.md` form (ADR 0028), which in an encrypted
    /// vault is *not* the filename. Completion inserts this; anything that
    /// opens the note uses `path`.
    pub link_target: String,
}

/// Lazily-populated note metadata, one bucket per vault root.
#[derive(Debug, Default)]
pub struct Cache {
    by_root: HashMap<PathBuf, Vec<CacheEntry>>,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached entries for a vault, scanning it on first use.
    ///
    /// Entries keep the scan's newest-first order (ADR 0025). Malformed notes
    /// are skipped silently, as the server has no `--strict` channel to report
    /// them.
    pub fn entries(&mut self, vault: &Vault) -> &[CacheEntry] {
        let root = vault.root().to_path_buf();
        if !self.by_root.contains_key(&root) {
            match scan_entries(vault) {
                // A locked vault is not cached. Storing its empty result would
                // mean an `ntropy unlock` in another terminal never took
                // effect here, because nothing would ever invalidate an entry
                // that looks like a legitimately empty vault.
                Scanned::Locked => return &[],
                Scanned::Entries(entries) => {
                    self.by_root.insert(root.clone(), entries);
                }
            }
        }
        self.by_root.get(&root).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every cached entry across all populated vault roots.
    ///
    /// Used by workspace symbols, which have no document context, so they range
    /// over whatever vaults the session has already touched.
    pub fn all_entries(&self) -> Vec<&CacheEntry> {
        self.by_root.values().flatten().collect()
    }

    /// Drop a vault's cached entries so the next access rescans it.
    pub fn invalidate(&mut self, root: &Path) {
        self.by_root.remove(root);
    }

    /// Whether a vault root currently has cached entries (test seam).
    #[cfg(test)]
    pub fn is_populated(&self, root: &Path) -> bool {
        self.by_root.contains_key(root)
    }
}

/// What a scan produced, keeping "locked" distinct from "empty".
enum Scanned {
    Entries(Vec<CacheEntry>),
    /// The vault is encrypted and no identity was available.
    Locked,
}

/// Scan a vault's `all-notes/` into cache entries.
///
/// The identity is fetched fresh every time rather than held: the cache is
/// rebuilt on every watched-file change anyway, a credential-store read is
/// negligible beside a full rescan, and caching the key would mean an
/// `ntropy lock` elsewhere never took effect here (ADR 0041).
fn scan_entries(vault: &Vault) -> Scanned {
    let session = match open_session(vault) {
        Some(session) => session,
        None => return Scanned::Locked,
    };
    let Ok(scan) = scan::scan_notes_dir(&session.layout().all_notes(), session.cipher()) else {
        return Scanned::Entries(Vec::new());
    };
    Scanned::Entries(
        scan.notes
            .into_iter()
            .map(|note| CacheEntry {
                id: note.id,
                link_target: note.link_target(),
                title: note.title,
                tags: note.tags,
                path: note.path,
            })
            .collect(),
    )
}

/// Open a readable session over `vault`, or `None` when it is locked.
///
/// The server never prompts: it has no controlling terminal, and a language
/// server that blocked an editor waiting for a passphrase would be worse than
/// one that stays quiet until an unlock happens elsewhere.
#[cfg(feature = "encryption")]
pub(super) fn open_session(vault: &Vault) -> Option<VaultSession> {
    use std::sync::Arc;

    use ntropy::cipher::AgeCipher;
    use ntropy::keys::{self, Acquisition, store::KeyringStore};

    let layout = vault.layout();
    let Ok(Some(recipient)) = keys::read_recipient_at(&layout.identity_pub()) else {
        return Some(VaultSession::plaintext(vault.clone()));
    };

    let Ok(store) = KeyringStore::open() else {
        return None;
    };
    let identity_file = std::env::var_os("NTROPY_IDENTITY").map(std::path::PathBuf::from);
    let wrapped = layout.identity_file();
    let request = Acquisition {
        recipient: &recipient,
        wrapped_identity: &wrapped,
        identity_file: identity_file.as_deref(),
        passphrase_file: None,
        store: &store,
        prompt: None,
    };
    let identity = keys::acquire(&request).ok()??;
    Some(VaultSession::with_cipher(
        vault.clone(),
        Arc::new(AgeCipher::new(recipient, Some(identity))),
    ))
}

/// Without encryption support an encrypted vault cannot be opened at all.
#[cfg(not(feature = "encryption"))]
pub(super) fn open_session(vault: &Vault) -> Option<VaultSession> {
    if ntropy::vault::layout::is_encrypted(vault.root()) {
        return None;
    }
    Some(VaultSession::plaintext(vault.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn vault_with(notes: &[(&str, &str, &str)]) -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        for (ulid, slug, content) in notes {
            std::fs::write(
                root.join("all-notes").join(format!("{ulid}-{slug}.md")),
                content,
            )
            .expect("write note");
        }
        let vault = Vault::new(std::fs::canonicalize(root).expect("canonicalize"));
        (dir, vault)
    }

    #[test]
    fn populates_lazily_on_first_access() {
        let (_guard, vault) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let mut cache = Cache::new();
        assert!(!cache.is_populated(vault.root()));
        assert_eq!(cache.entries(&vault).len(), 1);
        assert!(cache.is_populated(vault.root()));
        assert_eq!(cache.entries(&vault)[0].title, "Alpha");
    }

    #[test]
    fn does_not_rescan_until_invalidated() {
        let (_guard, vault) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let mut cache = Cache::new();
        assert_eq!(cache.entries(&vault).len(), 1);

        // A note added out of band is invisible while the cache holds.
        std::fs::write(
            vault.layout().all_notes().join(format!("{ULID_B}-b.md")),
            "---\ntitle: Beta\n---\n",
        )
        .expect("write note");
        assert_eq!(cache.entries(&vault).len(), 1);

        // After invalidation the rescan sees it.
        cache.invalidate(vault.root());
        assert_eq!(cache.entries(&vault).len(), 2);
    }

    #[test]
    fn empty_vault_yields_no_entries() {
        let (_guard, vault) = vault_with(&[]);
        let mut cache = Cache::new();
        assert!(cache.entries(&vault).is_empty());
    }

    #[test]
    fn malformed_notes_are_excluded() {
        let (_guard, vault) = vault_with(&[
            (ULID_A, "a", "---\ntitle: Good\n---\n"),
            // Missing `title` is malformed and skipped by the scan.
            (ULID_B, "b", "---\ntags: [x]\n---\n"),
        ]);
        let mut cache = Cache::new();
        assert_eq!(cache.entries(&vault).len(), 1);
    }

    #[test]
    fn distinct_roots_are_cached_independently() {
        let (_a, vault_a) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let (_b, vault_b) = vault_with(&[
            (ULID_A, "x", "---\ntitle: X\n---\n"),
            (ULID_B, "y", "---\ntitle: Y\n---\n"),
        ]);
        let mut cache = Cache::new();
        assert_eq!(cache.entries(&vault_a).len(), 1);
        assert_eq!(cache.entries(&vault_b).len(), 2);
    }

    #[test]
    fn all_entries_spans_every_populated_root() {
        let (_a, vault_a) = vault_with(&[(ULID_A, "a", "---\ntitle: Alpha\n---\n")]);
        let (_b, vault_b) = vault_with(&[(ULID_B, "b", "---\ntitle: Beta\n---\n")]);
        let mut cache = Cache::new();
        assert!(cache.all_entries().is_empty());
        cache.entries(&vault_a);
        cache.entries(&vault_b);
        assert_eq!(cache.all_entries().len(), 2);
    }

    /// Caching an encrypted vault.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use ntropy::cipher::AgeCipher;
        use ntropy::crypto::age_io;
        use ntropy::session::VaultSession;
        use std::sync::{Arc, Mutex, MutexGuard};

        /// Serializes the tests that set `$NTROPY_IDENTITY`.
        ///
        /// The environment is process-wide, so two of these running at once
        /// would see each other's value. Rust marks `set_var` unsafe for
        /// exactly this reason.
        static ENV: Mutex<()> = Mutex::new(());

        /// Point the server's key lookup at `identity` for as long as the
        /// guard is held.
        struct EnvGuard(#[allow(dead_code)] MutexGuard<'static, ()>);

        impl EnvGuard {
            fn set(identity: Option<&std::path::Path>) -> Self {
                let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
                // SAFETY: the mutex makes this the only thread touching the
                // environment for the duration of the test.
                unsafe {
                    match identity {
                        Some(path) => std::env::set_var("NTROPY_IDENTITY", path),
                        None => std::env::remove_var("NTROPY_IDENTITY"),
                    }
                }
                Self(guard)
            }
        }

        impl Drop for EnvGuard {
            fn drop(&mut self) {
                // SAFETY: still holding the mutex.
                unsafe { std::env::remove_var("NTROPY_IDENTITY") };
            }
        }

        /// An encrypted vault holding one note, plus the key that opens it.
        fn encrypted_vault_with_note() -> (tempfile::TempDir, Vault, PathBuf) {
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
            let note = session.layout().all_notes().join(format!("{ULID_A}.age"));
            session
                .cipher()
                .write(&note, "---\ntitle: Alpha\n---\nbody\n")
                .expect("write note");

            // The identity file lets the cache open the vault without a
            // keychain, which is also how a headless test avoids one.
            let identity_path = root.join("identity.txt");
            std::fs::write(&identity_path, format!("{}\n", identity.expose_secret()))
                .expect("identity file");

            let vault = Vault::new(std::fs::canonicalize(root).expect("canonicalize"));
            (dir, vault, identity_path)
        }

        #[test]
        fn a_locked_vault_is_not_cached_as_empty() {
            // Caching the empty result would mean an `ntropy unlock` in another
            // terminal never took effect, because nothing would invalidate an
            // entry indistinguishable from a legitimately empty vault.
            let (_guard, vault, identity) = encrypted_vault_with_note();
            let mut cache = Cache::new();

            // Locked: no identity anywhere the server can reach.
            let env = EnvGuard::set(None);
            assert!(cache.entries(&vault).is_empty());
            assert!(
                !cache.is_populated(vault.root()),
                "a locked vault must leave the cache untouched"
            );
            drop(env);

            // Unlocked elsewhere: the very next access sees the notes.
            let _env = EnvGuard::set(Some(&identity));
            assert_eq!(cache.entries(&vault).len(), 1);
        }

        #[test]
        fn an_unlocked_vault_yields_its_notes() {
            let (_guard, vault, identity) = encrypted_vault_with_note();
            let _env = EnvGuard::set(Some(&identity));

            let mut cache = Cache::new();
            let entries = cache.entries(&vault);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].title, "Alpha");
            // The on-disk name is the ULID alone; the link target carries the
            // slug, and completion inserts the latter.
            assert_eq!(
                entries[0].path.file_name().expect("name"),
                format!("{ULID_A}.age").as_str()
            );
            assert_eq!(entries[0].link_target, format!("{ULID_A}-alpha.md"));
        }
    }
}
