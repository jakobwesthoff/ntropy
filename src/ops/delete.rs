// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The delete use case (ADR 0018).
//!
//! Removes a note's canonical file and then refreshes the views, so the deleted
//! note's links disappear from every view tree. Selector resolution and the
//! confirmation prompt live in the binary; this is the headless effect.

use std::path::Path;

use crate::error::Result;
use crate::fsutil;
use crate::reconcile;
use crate::scan::ScanWarning;
use crate::session::VaultSession;

/// Delete the note file at `path` and sync the views.
///
/// Returns the scan warnings produced while syncing (so a caller can honor
/// `--strict`).
pub fn delete_note(session: &VaultSession, path: &Path) -> Result<Vec<ScanWarning>> {
    fsutil::remove_file(path)?;
    reconcile::refresh_views(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{vault_with_view, write_note};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn removes_file_and_prunes_links() {
        let (_g, session) = vault_with_view();
        let path = write_note(
            &session,
            &format!("{ULID}-note.md"),
            "---\ntitle: Note\ntags: [work]\n---\n",
        );

        // Build the view first so there is a link to prune.
        reconcile::refresh_views(&session).expect("refresh");
        assert!(session.root().join("by-tag/work").is_dir());

        delete_note(&session, &path).expect("delete");
        assert!(!path.exists());
        // The note's group is gone after the sync.
        assert!(!session.root().join("by-tag/work").exists());
    }
}
