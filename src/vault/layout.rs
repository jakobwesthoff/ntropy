// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Vault paths and reserved names (ADR 0007): the single source of truth.
//!
//! Every well-known location inside a vault is computed here, so no other
//! module hardcodes a string like `all-notes` or `.ntropy`. The reserved-name
//! set (which a view must not collide with) also lives here.

use std::path::{Path, PathBuf};

/// The canonical notes directory, the lossless source of truth.
pub const ALL_NOTES_DIR: &str = "all-notes";
/// The reserved per-vault configuration/templates directory.
pub const NTROPY_DIR: &str = ".ntropy";
/// The per-vault config file, inside [`NTROPY_DIR`].
pub const CONFIG_FILE: &str = "config.toml";
/// The templates directory, inside [`NTROPY_DIR`].
pub const TEMPLATES_DIR: &str = "templates";
/// The default template file, inside [`TEMPLATES_DIR`].
pub const DEFAULT_TEMPLATE_FILE: &str = "default.md";
/// The daily-note template file, inside [`TEMPLATES_DIR`].
pub const TODAY_TEMPLATE_FILE: &str = "today.md";
/// The project-local vault pointer file looked for during walk-up (ADR 0026).
pub const POINTER_FILE: &str = ".ntropy-vault";
/// The auto-managed root ignore file listing the derived view directories.
pub const GITIGNORE_FILE: &str = ".gitignore";
/// The root README explaining the vault to someone who discovers it.
pub const README_FILE: &str = "README.md";
/// The public age recipient of an encrypted vault, inside [`NTROPY_DIR`].
///
/// Its presence is what marks a vault as encrypted, so detection needs no
/// stored state anywhere (ADR 0041).
pub const IDENTITY_PUB_FILE: &str = "identity.pub";
/// The passphrase-wrapped vault identity, inside [`NTROPY_DIR`].
pub const IDENTITY_FILE: &str = "identity.age";
/// The marker recording a whole-vault conversion in progress, inside
/// [`NTROPY_DIR`].
pub const MIGRATION_FILE: &str = "migration.toml";

/// Top-level names reserved by ntropy; a view directory may use none of them.
///
/// `.gitignore` and `README.md` are reserved alongside the canonical
/// directories so a view can never be named after, and thereby clobber, a
/// file ntropy manages.
pub const RESERVED_NAMES: [&str; 4] = [ALL_NOTES_DIR, NTROPY_DIR, GITIGNORE_FILE, README_FILE];

/// Computes the well-known paths of a vault rooted at a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    /// Wrap a vault root. The root is taken as-is; resolution and validation
    /// happen in [`super::resolve`].
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Layout { root: root.into() }
    }

    /// The vault root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<root>/all-notes`.
    pub fn all_notes(&self) -> PathBuf {
        self.root.join(ALL_NOTES_DIR)
    }

    /// `<root>/.ntropy`.
    pub fn ntropy_dir(&self) -> PathBuf {
        self.root.join(NTROPY_DIR)
    }

    /// `<root>/.ntropy/config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.ntropy_dir().join(CONFIG_FILE)
    }

    /// `<root>/.ntropy/templates`.
    pub fn templates_dir(&self) -> PathBuf {
        self.ntropy_dir().join(TEMPLATES_DIR)
    }

    /// `<root>/.ntropy/templates/default.md`.
    pub fn default_template(&self) -> PathBuf {
        self.templates_dir().join(DEFAULT_TEMPLATE_FILE)
    }

    /// `<root>/.ntropy/templates/today.md`.
    pub fn today_template(&self) -> PathBuf {
        self.templates_dir().join(TODAY_TEMPLATE_FILE)
    }

    /// The output directory of a named view: `<root>/<name>`.
    pub fn view_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// `<root>/.gitignore`, the auto-managed ignore file for view directories.
    pub fn gitignore_file(&self) -> PathBuf {
        self.root.join(GITIGNORE_FILE)
    }

    /// `<root>/README.md`, the vault README seeded by `init`.
    pub fn readme_file(&self) -> PathBuf {
        self.root.join(README_FILE)
    }

    /// `<root>/.ntropy/identity.pub`, the public recipient of an encrypted
    /// vault.
    pub fn identity_pub(&self) -> PathBuf {
        self.ntropy_dir().join(IDENTITY_PUB_FILE)
    }

    /// `<root>/.ntropy/identity.age`, the wrapped identity of an encrypted
    /// vault.
    pub fn identity_file(&self) -> PathBuf {
        self.ntropy_dir().join(IDENTITY_FILE)
    }

    /// `<root>/.ntropy/migration.toml`, present only while a whole-vault
    /// conversion is under way.
    pub fn migration_file(&self) -> PathBuf {
        self.ntropy_dir().join(MIGRATION_FILE)
    }
}

/// Whether `path` looks like a vault: it contains a `.ntropy/` directory.
pub fn is_vault(path: &Path) -> bool {
    path.join(NTROPY_DIR).is_dir()
}

/// Whether the vault rooted at `path` stores its notes encrypted.
///
/// The test is the presence of the recipient file and nothing else, which
/// keeps detection stateless: no configuration to consult, nothing to fall out
/// of sync with the notes on disk. `is_file` rather than `exists` so a
/// directory that happens to carry the name is not mistaken for a recipient.
pub fn is_encrypted(path: &Path) -> bool {
    path.join(NTROPY_DIR).join(IDENTITY_PUB_FILE).is_file()
}

/// Whether `name` is a reserved top-level name a view may not use.
pub fn is_reserved_name(name: &str) -> bool {
    RESERVED_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_well_known_paths() {
        let layout = Layout::new("/vault");
        assert_eq!(layout.all_notes(), PathBuf::from("/vault/all-notes"));
        assert_eq!(layout.ntropy_dir(), PathBuf::from("/vault/.ntropy"));
        assert_eq!(
            layout.config_file(),
            PathBuf::from("/vault/.ntropy/config.toml")
        );
        assert_eq!(
            layout.default_template(),
            PathBuf::from("/vault/.ntropy/templates/default.md")
        );
        assert_eq!(layout.view_dir("by-tag"), PathBuf::from("/vault/by-tag"));
        assert_eq!(layout.gitignore_file(), PathBuf::from("/vault/.gitignore"));
        assert_eq!(layout.readme_file(), PathBuf::from("/vault/README.md"));
    }

    #[test]
    fn computes_encryption_paths() {
        let layout = Layout::new("/vault");
        assert_eq!(
            layout.identity_pub(),
            PathBuf::from("/vault/.ntropy/identity.pub")
        );
        assert_eq!(
            layout.identity_file(),
            PathBuf::from("/vault/.ntropy/identity.age")
        );
        assert_eq!(
            layout.migration_file(),
            PathBuf::from("/vault/.ntropy/migration.toml")
        );
    }

    #[test]
    fn is_encrypted_checks_for_the_recipient_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join(NTROPY_DIR)).expect("mkdir");
        assert!(!is_encrypted(dir.path()));

        std::fs::write(
            dir.path().join(NTROPY_DIR).join(IDENTITY_PUB_FILE),
            "age1example\n",
        )
        .expect("write recipient");
        assert!(is_encrypted(dir.path()));
    }

    #[test]
    fn a_directory_named_like_the_recipient_file_is_not_a_recipient() {
        // `is_file` rather than `exists`: a directory carrying the name would
        // otherwise flip a whole vault into a storage form it cannot read.
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join(NTROPY_DIR).join(IDENTITY_PUB_FILE))
            .expect("mkdir");
        assert!(!is_encrypted(dir.path()));
    }

    #[test]
    fn a_vault_without_an_ntropy_dir_is_not_encrypted() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(!is_encrypted(dir.path()));
    }

    #[test]
    fn the_identity_files_live_inside_the_reserved_directory() {
        // They need no entry in `RESERVED_NAMES` precisely because `.ntropy`
        // is already reserved; this pins that reasoning.
        let layout = Layout::new("/vault");
        assert!(layout.identity_pub().starts_with(layout.ntropy_dir()));
        assert!(layout.identity_file().starts_with(layout.ntropy_dir()));
        assert!(layout.migration_file().starts_with(layout.ntropy_dir()));
    }

    #[test]
    fn is_vault_checks_for_ntropy_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(!is_vault(dir.path()));
        std::fs::create_dir_all(dir.path().join(NTROPY_DIR)).expect("mkdir");
        assert!(is_vault(dir.path()));
    }

    #[test]
    fn reserved_names_cover_canonical_dirs() {
        assert!(is_reserved_name("all-notes"));
        assert!(is_reserved_name(".ntropy"));
        assert!(is_reserved_name(".gitignore"));
        assert!(is_reserved_name("README.md"));
        assert!(!is_reserved_name("by-tag"));
    }
}
