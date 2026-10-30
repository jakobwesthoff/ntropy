// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The view administration use cases: `view list|add|remove` (ADR 0018).
//!
//! These read and write the per-vault config and keep the derived state in
//! step: adding a view materializes it from the current notes, and both add and
//! remove sync the root `.gitignore` to the configured views (ADR 0032).
//! Removing a view never deletes its directory — ntropy leaves the now-stale
//! tree in place for the user to remove, pruning only the managed ignore entry.
//! Editing a view is intentionally absent in v1 (it is remove + add).

use crate::config::{PerVaultConfig, ViewConfig};
use crate::error::Result;
use crate::fsutil;
use crate::gitignore;
use crate::reconcile;
use crate::scan;
use crate::session::VaultSession;
use crate::vault::{InvalidViewName, Vault, ViewName};
use crate::view::{self, SkippedView, ViewDef};

/// Why a view administration command was rejected.
#[derive(Debug, thiserror::Error)]
pub enum ViewAdminError {
    /// The name would not stay a view directory of its own (ADR 0064).
    #[error(
        "`{}` is not a valid view name: {reason}{}",
        .name.escape_debug(),
        suggestion_hint(.suggestion)
    )]
    InvalidName {
        name: String,
        reason: InvalidViewName,
        suggestion: Option<ViewName>,
    },
    #[error("a view named `{0}` already exists")]
    Duplicate(String),
    #[error("no view named `{0}`")]
    NotFound(String),
    #[error(
        "materialized views are disabled in an encrypted vault: a symlink tree \
         would spell out the taxonomy in plaintext names inside the synced \
         directory"
    )]
    ViewsDisabledWhenEncrypted,
}

/// The " (try `x`)" tail of an invalid-name message, when there is a
/// suggestion.
fn suggestion_hint(suggestion: &Option<ViewName>) -> String {
    match suggestion {
        Some(name) => format!(" (try `{name}`)"),
        None => String::new(),
    }
}

/// What adding or removing a view did to the rest of the configuration.
#[derive(Debug, Default)]
pub struct ViewConfigSync {
    /// How `.gitignore` changed.
    pub gitignore: gitignore::SyncReport,
    /// Other configured views that are skipped because of their name.
    pub skipped_views: Vec<SkippedView>,
}

/// List the configured views.
///
/// Every entry is listed as configured, valid name or not, so a damaged
/// config can be inspected.
pub fn list_views(vault: &Vault) -> Result<Vec<ViewConfig>> {
    let config = PerVaultConfig::load(&vault.layout().config_file())?;
    Ok(config.views)
}

/// Add a view named `name` grouping by `field`, then materialize it.
pub fn add_view(session: &VaultSession, name: &str, field: &str) -> Result<ViewConfigSync> {
    // Refused rather than accepted-and-inert: a command that reports success
    // and produces no directory is the worst outcome for a user's mental
    // model, and erroring leaves room for views to return later if they are
    // ever relocated outside the vault. `list` and `remove` keep working, so a
    // vault encrypted after the fact can still be inspected and cleaned up.
    if session.is_encrypted() {
        return Err(ViewAdminError::ViewsDisabledWhenEncrypted.into());
    }
    // Validated before anything is written: the name becomes the directory
    // the sync below empties of everything that is not a view link.
    let view_name = ViewName::parse(name).map_err(|reason| ViewAdminError::InvalidName {
        name: name.to_string(),
        reason,
        suggestion: ViewName::suggest(name),
    })?;

    let config_path = session.layout().config_file();
    let mut config = PerVaultConfig::load(&config_path)?;
    let added = config.add(ViewConfig {
        name: name.to_string(),
        field: field.to_string(),
    });
    if !added {
        return Err(ViewAdminError::Duplicate(name.to_string()).into());
    }
    fsutil::atomic_write(&config_path, config.to_toml()?.as_bytes())?;

    // Materialize the new view immediately so its directory reflects the
    // current notes.
    let scan = scan::scan_notes_dir(&session.layout().all_notes(), session.cipher())?;
    view::sync_view(session, &ViewDef::new(view_name, field), &scan.notes)?;

    // Keep `.gitignore` in step with the now-larger view set.
    sync_gitignore(session, &config)
}

/// Remove the view named `name` from config and prune its `.gitignore` entry.
///
/// Works by the name as configured, valid or not: it only edits the config
/// and never builds a path from `name`, so it is how a view with an invalid
/// name is cleaned up.
///
/// The view's directory is intentionally left on disk; ntropy never deletes a
/// directory. The returned report names the pruned entry so the caller can tell
/// the user the directory remains.
pub fn remove_view(vault: &Vault, name: &str) -> Result<ViewConfigSync> {
    let config_path = vault.layout().config_file();
    let mut config = PerVaultConfig::load(&config_path)?;
    if !config.remove(name) {
        return Err(ViewAdminError::NotFound(name.to_string()).into());
    }
    fsutil::atomic_write(&config_path, config.to_toml()?.as_bytes())?;
    sync_gitignore(vault, &config)
}

/// Sync the root `.gitignore` to the views in `config`.
fn sync_gitignore(vault: &Vault, config: &PerVaultConfig) -> Result<ViewConfigSync> {
    let (views, skipped_views) = reconcile::configured_views(config);
    let gitignore = reconcile::sync_gitignore(vault, &views, &skipped_views)?;
    Ok(ViewConfigSync {
        gitignore,
        skipped_views,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn temp_vault() -> (tempfile::TempDir, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect(".ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));
        (dir, session)
    }

    #[test]
    fn add_then_list() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-status", "status").expect("add");
        let views = list_views(&v).expect("list");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].name, "by-status");
        assert!(v.root().join("by-status").is_dir());
    }

    #[test]
    fn add_writes_gitignore_entry() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-status", "status").expect("add");
        let gitignore = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        assert!(gitignore.contains("/by-status/"), "got: {gitignore}");
    }

    #[test]
    fn add_materializes_existing_notes() {
        let (_g, v) = temp_vault();
        std::fs::write(
            v.layout().all_notes().join(format!("{ULID}-n.md")),
            "---\ntitle: N\nstatus: Done\n---\n",
        )
        .expect("write note");
        add_view(&v, "by-status", "status").expect("add");
        assert!(v.root().join("by-status/done").is_dir());
    }

    #[test]
    fn add_rejects_reserved_name() {
        let (_g, v) = temp_vault();
        let err = add_view(&v, "all-notes", "tags").expect_err("reserved");
        assert!(matches!(
            err,
            Error::ViewAdmin(ViewAdminError::InvalidName {
                reason: InvalidViewName::Reserved(_),
                ..
            })
        ));
    }

    #[test]
    fn add_rejects_readme_as_view_name() {
        let (_g, v) = temp_vault();
        let err = add_view(&v, "README.md", "tags").expect_err("reserved");
        assert!(matches!(
            err,
            Error::ViewAdmin(ViewAdminError::InvalidName {
                reason: InvalidViewName::Reserved(_),
                ..
            })
        ));
    }

    /// A vault at `<base>/vault`, so a name that climbs out of the vault
    /// still lands inside the test's own temp dir.
    fn nested_vault() -> (tempfile::TempDir, std::path::PathBuf, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().to_path_buf();
        let root = base.join("vault");
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        (dir, base, VaultSession::plaintext(Vault::new(root)))
    }

    #[test]
    fn add_rejects_names_that_would_leave_the_view_directory() {
        let (guard, base, v) = nested_vault();
        let absolute = base.join("absolute");
        let absolute = absolute.to_str().expect("temp paths are UTF-8");
        let config_before = std::fs::read_to_string(v.layout().config_file()).ok();

        for name in [
            absolute,
            "../escape",
            ".git",
            ".ntropy",
            "all-notes",
            "all-notes/x",
            "a//b",
            "",
            "a\nb",
            "By_Tag",
        ] {
            let err = add_view(&v, name, "tags").expect_err("an invalid name is refused");
            assert!(
                matches!(err, Error::ViewAdmin(ViewAdminError::InvalidName { .. })),
                "`{}`: {err:?}",
                name.escape_debug()
            );
        }

        // Nothing was written: no config entry, no directory, no ignore file.
        assert_eq!(
            std::fs::read_to_string(v.layout().config_file()).ok(),
            config_before
        );
        assert!(!base.join("absolute").exists());
        assert!(!base.join("escape").exists());
        assert!(!v.root().join(".git").exists());
        assert!(!v.root().join("all-notes").join("x").exists());
        assert!(!v.root().join("By_Tag").exists());
        assert!(!v.layout().gitignore_file().exists());
        assert!(base.starts_with(guard.path()));
    }

    #[test]
    fn add_accepts_slug_names_and_nesting() {
        let (_g, v) = temp_vault();
        for name in ["by-tag", "by-status", "area/work"] {
            add_view(&v, name, "tags").expect("a valid name is accepted");
            assert!(v.root().join(name).is_dir(), "{name}");
        }
    }

    #[test]
    fn the_invalid_name_error_offers_a_suggestion() {
        let (_g, v) = temp_vault();
        let err = add_view(&v, "By_Tag", "tags").expect_err("invalid");
        assert_eq!(
            err.to_string(),
            "`By_Tag` is not a valid view name: the segment `By_Tag` may only hold \
             lowercase letters, digits and single dashes, at most 72 characters (try `bytag`)"
        );
        let err = add_view(&v, "all-notes", "tags").expect_err("invalid");
        assert_eq!(
            err.to_string(),
            "`all-notes` is not a valid view name: `all-notes` is reserved by ntropy"
        );
    }

    /// Write a config holding the given `(name, field)` views verbatim.
    fn configure(v: &Vault, views: &[(&str, &str)]) {
        let mut config = PerVaultConfig::default();
        for (name, field) in views {
            config.add(ViewConfig {
                name: (*name).into(),
                field: (*field).into(),
            });
        }
        std::fs::write(v.layout().config_file(), config.to_toml().expect("toml"))
            .expect("write config");
    }

    #[test]
    fn add_reports_other_views_that_are_skipped() {
        let (_g, base, v) = nested_vault();
        std::fs::create_dir_all(base.join("decoy")).expect("decoy");
        std::fs::write(base.join("decoy/keep.txt"), "keep").expect("decoy file");
        configure(&v, &[("../decoy", "tags")]);

        let sync = add_view(&v, "bytag", "tags").expect("add");
        let skipped: Vec<&str> = sync.skipped_views.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(skipped, ["../decoy"]);
        assert!(base.join("decoy/keep.txt").exists());
    }

    #[test]
    fn list_and_remove_work_on_invalid_names() {
        let (_g, base, v) = nested_vault();
        std::fs::create_dir_all(base.join("decoy")).expect("decoy");
        std::fs::write(base.join("decoy/keep.txt"), "keep").expect("decoy file");
        configure(&v, &[("../decoy", "tags"), ("By_Tag", "tags")]);

        let names: Vec<String> = list_views(&v)
            .expect("list")
            .into_iter()
            .map(|view| view.name)
            .collect();
        assert_eq!(names, ["../decoy", "By_Tag"]);

        let sync = remove_view(&v, "../decoy").expect("remove");
        let skipped: Vec<&str> = sync.skipped_views.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(skipped, ["By_Tag"]);
        remove_view(&v, "By_Tag").expect("remove");
        assert!(list_views(&v).expect("list").is_empty());
        assert!(base.join("decoy/keep.txt").exists());
    }

    #[test]
    fn add_rejects_duplicate() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-tag", "tags").expect("add");
        let err = add_view(&v, "by-tag", "tags").expect_err("dup");
        assert!(matches!(
            err,
            Error::ViewAdmin(ViewAdminError::Duplicate(_))
        ));
    }

    #[test]
    fn remove_keeps_directory_and_prunes_config_and_gitignore() {
        let (_g, v) = temp_vault();
        add_view(&v, "by-tag", "tags").expect("add");
        assert!(v.root().join("by-tag").is_dir());

        let report = remove_view(&v, "by-tag").expect("remove");
        assert!(list_views(&v).expect("list").is_empty());
        // ntropy never deletes the directory; the stale tree is left in place.
        assert!(v.root().join("by-tag").exists());
        // Only the managed ignore entry is pruned.
        assert_eq!(report.gitignore.removed, ["/by-tag/"]);
        let gitignore = std::fs::read_to_string(v.layout().gitignore_file()).expect("read");
        assert!(
            !gitignore.contains("/by-tag/"),
            "entry not pruned: {gitignore}"
        );
    }

    #[test]
    fn remove_unknown_is_error() {
        let (_g, v) = temp_vault();
        let err = remove_view(&v, "ghost").expect_err("missing");
        assert!(matches!(err, Error::ViewAdmin(ViewAdminError::NotFound(_))));
    }

    /// View administration against a vault whose notes are encrypted.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use super::*;
        use crate::test_support::encrypted::encrypted_vault;

        #[test]
        fn add_is_refused() {
            // Accepting it would write a definition that produces nothing
            // observable, which is worse for a user than a clear refusal.
            let fixture = encrypted_vault();
            let err = add_view(&fixture.session, "by-tag", "tags").expect_err("refused");
            assert!(matches!(
                err,
                Error::ViewAdmin(ViewAdminError::ViewsDisabledWhenEncrypted)
            ));
        }

        #[test]
        fn an_invalid_name_gets_the_encryption_refusal() {
            // The encryption check comes first: whatever the name, views are
            // unavailable here, and that is the actionable message.
            let fixture = encrypted_vault();
            let err = add_view(&fixture.session, "By_Tag", "tags").expect_err("refused");
            assert!(matches!(
                err,
                Error::ViewAdmin(ViewAdminError::ViewsDisabledWhenEncrypted)
            ));
        }

        #[test]
        fn a_refused_add_writes_no_config_and_no_directory() {
            let fixture = encrypted_vault();
            add_view(&fixture.session, "by-tag", "tags").expect_err("refused");
            assert!(list_views(&fixture.session).expect("list").is_empty());
            assert!(!fixture.session.root().join("by-tag").exists());
            assert!(!fixture.session.layout().gitignore_file().exists());
        }

        #[test]
        fn the_refusal_explains_why() {
            let message = ViewAdminError::ViewsDisabledWhenEncrypted.to_string();
            assert!(message.contains("encrypted vault"), "{message}");
            assert!(message.contains("plaintext"), "{message}");
        }

        #[test]
        fn list_and_remove_keep_working() {
            // A vault encrypted after the fact still carries definitions, so
            // they must remain inspectable and removable.
            let fixture = encrypted_vault();
            let config_path = fixture.session.layout().config_file();
            let mut config = PerVaultConfig::default();
            config.add(ViewConfig {
                name: "by-tag".into(),
                field: "tags".into(),
            });
            std::fs::write(&config_path, config.to_toml().expect("toml")).expect("write config");

            assert_eq!(list_views(&fixture.session).expect("list").len(), 1);
            remove_view(&fixture.session, "by-tag").expect("remove");
            assert!(list_views(&fixture.session).expect("list").is_empty());
        }
    }
}
