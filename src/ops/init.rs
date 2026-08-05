// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Vault initialization (ADR 0018).
//!
//! Scaffolds the directories and files a vault needs: `all-notes/`, `.ntropy/`,
//! the [seed files](crate::vault::seed), and a per-vault config seeded with a
//! `by-tag` view (plus that view's directory). It is idempotent: each missing piece is
//! created and each existing piece is left untouched, so re-running `init`
//! always succeeds. It never touches the global config; the binary writes the
//! default-vault entry separately when `--set-default` is passed.

use std::path::{Path, PathBuf};

use crate::cipher::Passphrase;
use crate::config::{PerVaultConfig, ViewConfig};
use crate::error::Result;
use crate::fsutil;
use crate::gitignore;
#[cfg(feature = "encryption")]
use crate::keys;
use crate::vault::{Layout, Vault, layout, seed};

/// The view seeded into a fresh vault.
const SEED_VIEW_NAME: &str = "by-tag";
const SEED_VIEW_FIELD: &str = "tags";

/// Resolves a seeded file's destination against a vault's layout.
type SeedPath = fn(&Layout) -> PathBuf;

/// The files a vault is seeded with: where each one goes, and what it holds.
///
/// Every entry is written only when absent, so a re-init restores whatever the
/// user deleted and preserves whatever they customized. Adding a seeded file
/// means adding it under `src/vault/seed/` and listing it here.
const SEEDED_FILES: &[(SeedPath, &str)] = &[
    (Layout::default_template, seed::DEFAULT_TEMPLATE),
    (Layout::today_template, seed::TODAY_TEMPLATE),
    (Layout::readme_file, seed::VAULT_README),
];

/// How a vault being initialized should store its notes.
#[derive(Debug, Default)]
pub struct InitOptions {
    /// Wrap a fresh age keypair under this passphrase, making the vault
    /// encrypted.
    ///
    /// The library never asks for a passphrase, so the binary has already
    /// obtained one by the time this is set (ADR 0013).
    pub encrypt_with: Option<Passphrase>,
}

/// What `init` did, for human-facing reporting.
#[derive(Debug, Default)]
pub struct InitReport {
    /// The resolved vault root.
    pub root: PathBuf,
    /// Pieces newly created by this run (empty on a re-init of a complete
    /// vault).
    pub created: Vec<PathBuf>,
    /// `.gitignore` entries added to match the configured views.
    pub gitignore_added: Vec<String>,
    /// `.gitignore` entries pruned because their view is no longer configured.
    pub gitignore_removed: Vec<String>,
    /// The vault's recipient, when it is encrypted.
    pub recipient: Option<String>,
}

/// Initialize (or complete) a plaintext vault rooted at `path`.
pub fn init_vault(path: &Path) -> Result<InitReport> {
    init_vault_with(path, &InitOptions::default())
}

/// Initialize (or complete) a vault rooted at `path`.
pub fn init_vault_with(path: &Path, options: &InitOptions) -> Result<InitReport> {
    let mut created = Vec::new();

    // The root and the well-known directories.
    ensure_dir(path, &mut created)?;
    let vault = Vault::new(path);
    let layout = vault.layout();

    ensure_dir(&layout.all_notes(), &mut created)?;
    ensure_dir(&layout.ntropy_dir(), &mut created)?;
    ensure_dir(&layout.templates_dir(), &mut created)?;

    // The keypair comes first, before anything that depends on knowing whether
    // this vault is encrypted. Re-running `init` on an encrypted vault must
    // never regenerate it: a new keypair would orphan every note already
    // written against the old one.
    let recipient = ensure_keypair(layout, options, &mut created)?;
    let encrypted = recipient.is_some() || layout::is_encrypted(path);

    for (path_of, contents) in SEEDED_FILES {
        ensure_file(&path_of(layout), contents, &mut created)?;
    }

    // The per-vault config, seeded with the `by-tag` view on first creation.
    //
    // An encrypted vault seeds no view: materialized views are disabled there,
    // and a definition that produces nothing would only be a puzzle for whoever
    // reads the config next.
    let config_path = layout.config_file();
    if !config_path.exists() {
        let mut config = PerVaultConfig::default();
        if !encrypted {
            config.add(ViewConfig {
                name: SEED_VIEW_NAME.into(),
                field: SEED_VIEW_FIELD.into(),
            });
        }
        fsutil::atomic_write(&config_path, config.to_toml()?.as_bytes())?;
        created.push(config_path);
        if !encrypted {
            // The seeded view's directory, empty until notes carry tags.
            ensure_dir(&layout.view_dir(SEED_VIEW_NAME), &mut created)?;
        }
    }

    // Keep the root .gitignore in step with the configured views so the derived
    // view directories stay out of version control (ADR 0032). Loading from
    // disk covers both a fresh init and completing an existing vault. An
    // encrypted vault has no views, so it gets no ignore file at all.
    let sync = if encrypted {
        gitignore::SyncReport::default()
    } else {
        let config = PerVaultConfig::load(&layout.config_file())?;
        let names: Vec<&str> = config.views.iter().map(|v| v.name.as_str()).collect();
        gitignore::sync(&vault, &names)?
    };

    Ok(InitReport {
        root: path.to_path_buf(),
        created,
        gitignore_added: sync.added,
        gitignore_removed: sync.removed,
        recipient,
    })
}

/// Write a fresh keypair when one is requested and none exists.
///
/// Returns the vault's recipient when this call created one. An existing
/// `identity.pub` is left strictly alone, so `init --encrypted` over an
/// encrypted vault completes it rather than replacing its key.
#[cfg(feature = "encryption")]
fn ensure_keypair(
    layout: &Layout,
    options: &InitOptions,
    created: &mut Vec<PathBuf>,
) -> Result<Option<String>> {
    let Some(passphrase) = options.encrypt_with.as_ref() else {
        return Ok(None);
    };
    if layout.identity_pub().is_file() {
        return Ok(None);
    }

    let (identity, recipient) = crate::crypto::age_io::generate_keypair();
    keys::write_keypair_at(
        &layout.identity_pub(),
        &layout.identity_file(),
        &identity,
        passphrase,
    )?;
    created.push(layout.identity_pub());
    created.push(layout.identity_file());
    Ok(Some(recipient.as_str()))
}

/// Refuse to create an encrypted vault in a build without encryption support.
#[cfg(not(feature = "encryption"))]
fn ensure_keypair(
    _layout: &Layout,
    options: &InitOptions,
    _created: &mut Vec<PathBuf>,
) -> Result<Option<String>> {
    if options.encrypt_with.is_some() {
        return Err(crate::cipher::CipherError::Unsupported.into());
    }
    Ok(None)
}

/// Create `dir` (and parents) if it does not yet exist, recording creation.
fn ensure_dir(dir: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        fsutil::create_dir_all(dir)?;
        created.push(dir.to_path_buf());
    }
    Ok(())
}

/// Write `contents` to `file` if it does not yet exist, recording creation.
fn ensure_file(file: &Path, contents: &str, created: &mut Vec<PathBuf>) -> Result<()> {
    if !file.exists() {
        fsutil::atomic_write(file, contents.as_bytes())?;
        created.push(file.to_path_buf());
    }
    Ok(())
}

/// Whether `path` is already an initialized vault.
pub fn is_initialized(path: &Path) -> bool {
    layout::is_vault(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffolds_a_fresh_vault() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("vault");
        let report = init_vault(&root).expect("init");

        let vault = Vault::new(&root);
        assert!(vault.layout().all_notes().is_dir());
        assert!(vault.layout().ntropy_dir().is_dir());
        assert!(vault.layout().templates_dir().is_dir());
        assert!(vault.layout().default_template().is_file());
        assert!(vault.layout().today_template().is_file());
        assert!(vault.layout().config_file().is_file());
        assert!(vault.layout().view_dir("by-tag").is_dir());
        assert!(vault.layout().readme_file().is_file());
        assert!(!report.created.is_empty());
    }

    #[test]
    fn readme_names_the_tooling_and_how_to_install_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let readme = std::fs::read_to_string(Vault::new(dir.path()).layout().readme_file())
            .expect("read README");
        assert!(
            readme.contains("https://ntropy.westhoffswelt.de"),
            "got: {readme}"
        );
        assert!(readme.contains("cargo install ntropy"), "got: {readme}");
    }

    #[test]
    fn re_init_restores_a_deleted_readme() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let readme = Vault::new(dir.path()).layout().readme_file();

        std::fs::remove_file(&readme).expect("remove");
        let report = init_vault(dir.path()).expect("re-init");
        assert!(readme.is_file());
        assert_eq!(report.created, [readme]);
    }

    #[test]
    fn re_init_preserves_a_user_edited_readme() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let readme = Vault::new(dir.path()).layout().readme_file();

        std::fs::write(&readme, "my own intro").expect("write");
        init_vault(dir.path()).expect("re-init");
        assert_eq!(std::fs::read_to_string(&readme).unwrap(), "my own intro");
    }

    #[test]
    fn writes_gitignore_for_the_seeded_view() {
        let dir = tempfile::tempdir().expect("temp dir");
        let report = init_vault(dir.path()).expect("init");
        assert_eq!(report.gitignore_added, ["/by-tag/"]);

        let gitignore = std::fs::read_to_string(Vault::new(dir.path()).layout().gitignore_file())
            .expect("read .gitignore");
        assert!(gitignore.contains("/by-tag/"), "got: {gitignore}");
    }

    #[test]
    fn re_init_completes_a_missing_gitignore() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let gitignore = Vault::new(dir.path()).layout().gitignore_file();

        // The user deleted the file; a re-init restores the managed entry.
        std::fs::remove_file(&gitignore).expect("remove");
        let report = init_vault(dir.path()).expect("re-init");
        assert_eq!(report.gitignore_added, ["/by-tag/"]);
        assert!(gitignore.exists());
    }

    #[test]
    fn re_init_preserves_a_user_edited_gitignore() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let gitignore = Vault::new(dir.path()).layout().gitignore_file();

        // Append a user line, then re-init: the line survives, no duplicate entry.
        let edited = format!("{}\n*.bak\n", std::fs::read_to_string(&gitignore).unwrap());
        std::fs::write(&gitignore, &edited).expect("write");
        let report = init_vault(dir.path()).expect("re-init");

        assert!(report.gitignore_added.is_empty());
        let after = std::fs::read_to_string(&gitignore).expect("read");
        assert!(after.contains("*.bak"), "user line lost: {after}");
        assert_eq!(after.matches("/by-tag/").count(), 1, "duplicated: {after}");
    }

    #[test]
    fn seeds_by_tag_view() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let config = PerVaultConfig::load(&Vault::new(dir.path()).layout().config_file())
            .expect("load config");
        assert_eq!(config.views.len(), 1);
        assert_eq!(config.views[0].name, "by-tag");
        assert_eq!(config.views[0].field, "tags");
    }

    #[test]
    fn default_template_is_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let template = std::fs::read_to_string(Vault::new(dir.path()).layout().default_template())
            .expect("read template");
        assert_eq!(template, seed::DEFAULT_TEMPLATE);
    }

    #[test]
    fn today_template_is_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let template = std::fs::read_to_string(Vault::new(dir.path()).layout().today_template())
            .expect("read template");
        assert_eq!(template, seed::TODAY_TEMPLATE);
    }

    #[test]
    fn is_idempotent() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("first init");
        // A second run creates nothing and still succeeds.
        let report = init_vault(dir.path()).expect("second init");
        assert!(report.created.is_empty());
        assert!(report.gitignore_added.is_empty());
        assert!(report.gitignore_removed.is_empty());
    }

    #[test]
    fn preserves_customized_template_and_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        init_vault(dir.path()).expect("init");
        let vault = Vault::new(dir.path());

        // Customize both, then re-init: customizations survive.
        std::fs::write(vault.layout().default_template(), "custom").expect("write template");
        let mut config = PerVaultConfig::default();
        config.add(ViewConfig {
            name: "by-status".into(),
            field: "status".into(),
        });
        std::fs::write(vault.layout().config_file(), config.to_toml().unwrap()).expect("write");

        init_vault(dir.path()).expect("re-init");
        assert_eq!(
            std::fs::read_to_string(vault.layout().default_template()).unwrap(),
            "custom"
        );
        let reloaded = PerVaultConfig::load(&vault.layout().config_file()).expect("reload");
        assert_eq!(reloaded.views[0].name, "by-status");
    }

    /// Initializing a vault whose notes are encrypted.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use crate::crypto::Recipient;
        use crate::keys;

        fn encrypted_options(passphrase: &str) -> InitOptions {
            InitOptions {
                encrypt_with: Some(Passphrase::new(passphrase)),
            }
        }

        #[test]
        fn writes_both_key_files() {
            let dir = tempfile::tempdir().expect("temp dir");
            let report = init_vault_with(dir.path(), &encrypted_options("p")).expect("init");

            let layout = Layout::new(dir.path());
            assert!(layout.identity_pub().is_file());
            assert!(layout.identity_file().is_file());
            assert!(report.recipient.is_some());
            assert!(layout::is_encrypted(dir.path()));
        }

        #[test]
        fn the_reported_recipient_matches_the_wrapped_identity() {
            let dir = tempfile::tempdir().expect("temp dir");
            let report = init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let layout = Layout::new(dir.path());

            let identity = keys::unwrap_identity_at(&layout.identity_file(), &Passphrase::new("p"))
                .expect("unwrap");
            assert_eq!(
                identity.to_public().as_str(),
                report.recipient.expect("recipient")
            );
        }

        #[test]
        fn the_recipient_file_holds_the_same_key() {
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let layout = Layout::new(dir.path());

            let recipient: Recipient = keys::read_recipient_at(&layout.identity_pub())
                .expect("read")
                .expect("present");
            let identity = keys::unwrap_identity_at(&layout.identity_file(), &Passphrase::new("p"))
                .expect("unwrap");
            assert_eq!(identity.to_public(), recipient);
        }

        #[test]
        fn re_init_never_regenerates_the_keypair() {
            // The most dangerous bug this feature could have: a second keypair
            // would orphan every note already written against the first, with
            // nothing on disk to say why they stopped decrypting.
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let layout = Layout::new(dir.path());

            let recipient_before = std::fs::read(layout.identity_pub()).expect("read");
            let wrapped_before = std::fs::read(layout.identity_file()).expect("read");

            // Even asked again, with a different passphrase.
            let report =
                init_vault_with(dir.path(), &encrypted_options("different")).expect("re-init");

            assert_eq!(
                std::fs::read(layout.identity_pub()).expect("read"),
                recipient_before
            );
            assert_eq!(
                std::fs::read(layout.identity_file()).expect("read"),
                wrapped_before
            );
            assert!(report.recipient.is_none(), "nothing new was created");
            // The original passphrase still opens it.
            assert!(
                keys::unwrap_identity_at(&layout.identity_file(), &Passphrase::new("p")).is_ok()
            );
        }

        #[test]
        fn a_plain_re_init_does_not_downgrade_an_encrypted_vault() {
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");

            init_vault(dir.path()).expect("re-init without the flag");
            assert!(layout::is_encrypted(dir.path()));
        }

        #[test]
        fn no_view_is_seeded() {
            // Views are disabled in an encrypted vault, so a definition here
            // would be a puzzle for whoever reads the config next.
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let layout = Layout::new(dir.path());

            let config = PerVaultConfig::load(&layout.config_file()).expect("load");
            assert!(config.views.is_empty());
            assert!(!layout.view_dir(SEED_VIEW_NAME).exists());
        }

        #[test]
        fn no_gitignore_is_written() {
            let dir = tempfile::tempdir().expect("temp dir");
            let report = init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            assert!(report.gitignore_added.is_empty());
            assert!(!Layout::new(dir.path()).gitignore_file().exists());
        }

        #[test]
        fn the_readme_and_templates_stay_plaintext() {
            // They are identical boilerplate in every vault and are not notes,
            // so encrypting them would hide nothing and cost readability.
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let layout = Layout::new(dir.path());

            assert_eq!(
                std::fs::read_to_string(layout.readme_file()).expect("read"),
                seed::VAULT_README
            );
            assert_eq!(
                std::fs::read_to_string(layout.default_template()).expect("read"),
                seed::DEFAULT_TEMPLATE
            );
        }

        #[test]
        fn all_notes_starts_empty() {
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let entries: Vec<_> = std::fs::read_dir(Layout::new(dir.path()).all_notes())
                .expect("read dir")
                .collect();
            assert!(entries.is_empty());
        }

        #[test]
        fn the_wrapped_identity_is_owner_only() {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault_with(dir.path(), &encrypted_options("p")).expect("init");
            let mode = std::fs::metadata(Layout::new(dir.path()).identity_file())
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "got {:o}", mode & 0o777);
        }

        #[test]
        fn an_empty_passphrase_is_refused_and_leaves_no_vault_key() {
            let dir = tempfile::tempdir().expect("temp dir");
            assert!(init_vault_with(dir.path(), &encrypted_options("")).is_err());
            assert!(!layout::is_encrypted(dir.path()));
        }

        #[test]
        fn a_plaintext_init_is_unchanged_by_any_of_this() {
            let dir = tempfile::tempdir().expect("temp dir");
            init_vault(dir.path()).expect("init");
            let layout = Layout::new(dir.path());

            assert!(!layout::is_encrypted(dir.path()));
            assert!(!layout.identity_pub().exists());
            let config = PerVaultConfig::load(&layout.config_file()).expect("load");
            assert_eq!(config.views.len(), 1);
            assert!(layout.view_dir(SEED_VIEW_NAME).is_dir());
            assert!(layout.gitignore_file().is_file());
        }
    }
}
