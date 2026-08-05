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
///
/// `$NTROPY_VAULT_HINT` is the fallback for a document that resolves to no
/// vault, which is what an encrypted vault's decrypted temp file always does:
/// it lives outside the vault by design. ntropy sets the variable when it
/// launches an editor, and the language server that editor starts inherits it
/// (ADR 0041).
pub fn for_document(uri: &Uri) -> Lookup {
    for_document_with_hint(
        uri,
        std::env::var_os("NTROPY_VAULT_HINT")
            .map(std::path::PathBuf::from)
            .as_deref(),
    )
}

/// The injectable core of [`for_document`], so the fallback is testable
/// without mutating the process environment.
pub fn for_document_with_hint(uri: &Uri, hint: Option<&std::path::Path>) -> Lookup {
    match resolve_from_document(uri) {
        // The hint only ever fills a gap. A document that resolves to its own
        // vault keeps it, so an editor started by ntropy in one vault still
        // works correctly on a document from another.
        Lookup::None => match hint {
            Some(root) if ntropy::vault::layout::is_vault(root) => Lookup::Found(Vault::new(root)),
            _ => Lookup::None,
        },
        found => found,
    }
}

/// Resolve strictly from the document's own location.
fn resolve_from_document(uri: &Uri) -> Lookup {
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

    #[test]
    fn a_hint_resolves_a_document_outside_any_vault() {
        // The case that matters: an encrypted vault's decrypted note is edited
        // through a file outside the vault, which resolves to nothing on its
        // own.
        let vault_dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(vault_dir.path().join(".ntropy")).expect(".ntropy");
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let document = elsewhere.path().join("01ARZ.md");
        std::fs::write(&document, "body").expect("write");

        let uri = uri::from_path(&document).expect("uri");
        match for_document_with_hint(&uri, Some(vault_dir.path())) {
            Lookup::Found(vault) => assert_eq!(vault.root(), vault_dir.path()),
            other => panic!("expected the hint to resolve, got {other:?}"),
        }
    }

    #[test]
    fn a_hint_never_overrides_a_document_that_resolves() {
        // An editor started by ntropy in one vault must still behave correctly
        // on a document belonging to another.
        let own = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(own.path().join(".ntropy")).expect(".ntropy");
        let hinted = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(hinted.path().join(".ntropy")).expect(".ntropy");

        let document = own.path().join("note.md");
        std::fs::write(&document, "body").expect("write");
        let uri = uri::from_path(&document).expect("uri");

        match for_document_with_hint(&uri, Some(hinted.path())) {
            Lookup::Found(vault) => assert_eq!(
                std::fs::canonicalize(vault.root()).expect("canonical"),
                std::fs::canonicalize(own.path()).expect("canonical")
            ),
            other => panic!("expected the document's own vault, got {other:?}"),
        }
    }

    #[test]
    fn a_hint_that_is_not_a_vault_is_ignored() {
        let not_a_vault = tempfile::tempdir().expect("temp dir");
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let document = elsewhere.path().join("note.md");
        std::fs::write(&document, "body").expect("write");

        let uri = uri::from_path(&document).expect("uri");
        assert!(matches!(
            for_document_with_hint(&uri, Some(not_a_vault.path())),
            Lookup::None
        ));
    }

    #[test]
    fn a_hint_pointing_nowhere_is_ignored() {
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let document = elsewhere.path().join("note.md");
        std::fs::write(&document, "body").expect("write");

        let uri = uri::from_path(&document).expect("uri");
        assert!(matches!(
            for_document_with_hint(&uri, Some(Path::new("/no/such/vault"))),
            Lookup::None
        ));
    }

    #[test]
    fn no_hint_leaves_an_unresolvable_document_unresolved() {
        let elsewhere = tempfile::tempdir().expect("temp dir");
        let document = elsewhere.path().join("note.md");
        std::fs::write(&document, "body").expect("write");

        let uri = uri::from_path(&document).expect("uri");
        assert!(matches!(for_document_with_hint(&uri, None), Lookup::None));
    }
}
