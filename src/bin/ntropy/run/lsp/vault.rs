// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Resolving the vault a document belongs to (ADR 0029).
//!
//! The language server starts without a vault and resolves one per document by
//! walking up from the document's directory, reusing the CLI's resolution rules.
//! A document outside any vault simply has no candidates; a broken
//! `.ntropy-vault` pointer is surfaced to the user rather than silently ignored,
//! mirroring the CLI's "misconfiguration is visible" stance.

use lsp_types::Uri;

use ntropy::vault::resolve::ResolveError;
use ntropy::vault::{ResolveOptions, Vault};

use super::uri;

/// The outcome of resolving a document's vault.
#[derive(Debug)]
pub enum Lookup {
    /// The document belongs to this vault.
    Found(Vault),
    /// The document is not inside any vault (or is not a `file:` document).
    None,
    /// A `.ntropy-vault` pointer is broken; the message should be shown.
    Broken(String),
}

/// Resolve the vault for a document URI.
pub fn for_document(uri: &Uri) -> Lookup {
    let Some(path) = uri::to_path(uri) else {
        return Lookup::None;
    };
    let start_dir = path.parent().map(|dir| dir.to_path_buf());
    let opts = ResolveOptions {
        start_dir,
        ..Default::default()
    };
    match Vault::resolve(&opts) {
        Ok(vault) => Lookup::Found(vault),
        Err(ResolveError::BrokenPointer { pointer, reason }) => {
            Lookup::Broken(format!("{}: {reason}", pointer.display()))
        }
        Err(ResolveError::NoVault | ResolveError::NotAVault(_)) => Lookup::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::str::FromStr;

    fn file_uri(path: &Path) -> Uri {
        Uri::from_str(&format!("file://{}", path.display())).expect("uri")
    }

    fn make_vault(root: &Path) {
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    }

    #[test]
    fn document_inside_a_vault_resolves() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path().join("v");
        make_vault(&vault);
        let doc = vault.join("all-notes").join("a.md");

        match for_document(&file_uri(&doc)) {
            Lookup::Found(found) => assert_eq!(
                found.root(),
                std::fs::canonicalize(&vault).expect("canonicalize")
            ),
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn document_outside_any_vault_is_none() {
        let dir = tempfile::tempdir().expect("temp dir");
        let doc = dir.path().join("loose.md");
        assert!(matches!(for_document(&file_uri(&doc)), Lookup::None));
    }

    #[test]
    fn non_file_uri_is_none() {
        let uri = Uri::from_str("untitled:Untitled-1").expect("uri");
        assert!(matches!(for_document(&uri), Lookup::None));
    }

    #[test]
    fn broken_pointer_is_reported() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).expect("project");
        std::fs::write(project.join(".ntropy-vault"), "./does-not-exist\n").expect("pointer");
        let doc = project.join("a.md");
        assert!(matches!(for_document(&file_uri(&doc)), Lookup::Broken(_)));
    }

    #[test]
    fn two_documents_in_one_vault_resolve_to_the_same_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path().join("v");
        make_vault(&vault);
        std::fs::create_dir_all(vault.join("sub")).expect("sub");
        let a = vault.join("all-notes").join("a.md");
        let b = vault.join("sub").join("b.md");

        let root = |uri| match for_document(&uri) {
            Lookup::Found(found) => found.root().to_path_buf(),
            other => panic!("expected Found, got {other:?}"),
        };
        assert_eq!(root(file_uri(&a)), root(file_uri(&b)));
    }
}
