// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The new-note use case (ADRs 0015, 0017).
//!
//! Generates an identity, derives the slug, renders the vault's default
//! template, and writes the canonical `all-notes/<ulid>-<slug>.md` file. View
//! links are refreshed separately by the caller after the (possible) editor
//! session, so this stays a pure create.

use crate::datetime;
use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::note::{Note, filename};
use crate::template::{self, TemplateVars};
use crate::text::slug;
use crate::vault::Vault;

/// Create a note titled `title` in `vault` from the default template.
///
/// Returns the parsed [`Note`], whose `path` is the file just written.
pub fn create_note(vault: &Vault, title: &str) -> Result<Note> {
    let id = Id::generate();
    let slug = slug::slugify(title);
    let date = datetime::render_local_date(id.timestamp_ms())?;

    let template = template::load_or_default(&vault.layout().default_template())?;
    let vars = TemplateVars {
        title: title.to_string(),
        id: id.to_string(),
        date,
        slug: slug.clone(),
    };
    let content = template::render(&template, &vars);

    // The canonical store must exist before the atomic write places a temp file
    // beside the destination; creating it is idempotent on an initialized vault.
    let all_notes = vault.layout().all_notes();
    fsutil::create_dir_all(&all_notes)?;

    let path = all_notes.join(filename::build(&id, &slug));
    fsutil::atomic_write(&path, content.as_bytes())?;

    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    Ok(Note::parse(path, &content, modified)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initialize just enough of a vault for `create_note` to run.
    fn temp_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect("mkdir .ntropy");
        let vault = Vault::new(dir.path());
        (dir, vault)
    }

    #[test]
    fn creates_file_with_canonical_name() {
        let (_guard, vault) = temp_vault();
        let note = create_note(&vault, "My First Note").expect("create");

        let name = note.path.file_name().unwrap().to_string_lossy();
        assert!(name.ends_with("-my-first-note.md"));
        assert_eq!(name.len(), 26 + "-my-first-note.md".len());
        assert!(note.path.exists());
    }

    #[test]
    fn rendered_content_round_trips_into_note() {
        let (_guard, vault) = temp_vault();
        let note = create_note(&vault, "My First Note").expect("create");
        assert_eq!(note.title, "My First Note");
        assert_eq!(note.tags, Vec::<String>::new());

        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("title: My First Note"));
        assert!(on_disk.contains("# My First Note"));
    }

    #[test]
    fn uses_custom_template_when_present() {
        let (_guard, vault) = temp_vault();
        let templates = vault.layout().templates_dir();
        std::fs::create_dir_all(&templates).expect("mkdir templates");
        std::fs::write(
            vault.layout().default_template(),
            "---\ntitle: {{title}}\n---\nCustom body for {{slug}}\n",
        )
        .expect("write template");

        let note = create_note(&vault, "Hello World").expect("create");
        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("Custom body for hello-world"));
    }

    #[test]
    fn untitled_fallback_for_empty_title() {
        let (_guard, vault) = temp_vault();
        let note = create_note(&vault, "???").expect("create");
        assert!(
            note.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-untitled.md")
        );
    }
}
