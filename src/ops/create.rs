// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The new-note use case (ADRs 0015, 0017).
//!
//! Generates an identity, derives the slug, renders a template, and writes the
//! canonical `all-notes/<ulid>-<slug>.md` file. The template is the vault's
//! `default.md` (with an embedded fallback) unless a name is given, in which
//! case `<name>.md` is required. View links are refreshed separately by the
//! caller after the (possible) editor session, so this stays a pure create.

use crate::datetime;
use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::note::{Note, filename};
use crate::scan;
use crate::template::{self, TemplateVars};
use crate::text::slug;
use crate::vault::Vault;

/// Create a note titled `title` in `vault` from a template.
///
/// `template` selects `<name>.md` from the vault's templates directory; `None`
/// uses `default.md` (falling back to the embedded default when absent). Returns
/// the parsed [`Note`], whose `path` is the file just written.
pub fn create_note(vault: &Vault, title: &str, template: Option<&str>) -> Result<Note> {
    let id = Id::generate();
    let slug = slug::slugify(title);
    let date = datetime::render_local_date(id.timestamp_ms())?;

    let template = match template {
        None => template::load_or_default(&vault.layout().default_template())?,
        Some(name) => template::load_named(&vault.layout().templates_dir(), name)?,
    };
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

/// The outcome of resolving today's note: the note plus whether it was created.
#[derive(Debug)]
pub struct TodayOutcome {
    /// Today's note, freshly created or the pre-existing one.
    pub note: Note,
    /// `true` when this call created the note, `false` when it already existed.
    pub created: bool,
}

/// Find today's note, or create it from the `today` template.
///
/// "Today's note" is the note whose title is today's local date. When several
/// match (an unlikely manual duplicate), the newest is returned, since the scan
/// is newest-first. When none exists it is created from `today.md`, which must be
/// present (`init` seeds it).
pub fn today_note(vault: &Vault) -> Result<TodayOutcome> {
    let date = datetime::today_local_date();

    // A vault that has not created any note yet has no `all-notes/`; treat that
    // as "no match" rather than scanning a missing directory.
    let all_notes = vault.layout().all_notes();
    if all_notes.is_dir() {
        let scan = scan::scan_notes_dir(&all_notes)?;
        if let Some(existing) = scan.notes.into_iter().find(|n| n.title == date) {
            return Ok(TodayOutcome {
                note: existing,
                created: false,
            });
        }
    }

    let note = create_note(vault, &date, Some("today"))?;
    Ok(TodayOutcome {
        note,
        created: true,
    })
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
        let note = create_note(&vault, "My First Note", None).expect("create");

        let name = note.path.file_name().unwrap().to_string_lossy();
        assert!(name.ends_with("-my-first-note.md"));
        assert_eq!(name.len(), 26 + "-my-first-note.md".len());
        assert!(note.path.exists());
    }

    #[test]
    fn rendered_content_round_trips_into_note() {
        let (_guard, vault) = temp_vault();
        let note = create_note(&vault, "My First Note", None).expect("create");
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

        let note = create_note(&vault, "Hello World", None).expect("create");
        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("Custom body for hello-world"));
    }

    #[test]
    fn uses_named_template_when_selected() {
        let (_guard, vault) = temp_vault();
        let templates = vault.layout().templates_dir();
        std::fs::create_dir_all(&templates).expect("mkdir templates");
        std::fs::write(
            templates.join("meeting.md"),
            "---\ntitle: {{title}}\ntags: [meeting]\n---\nAgenda for {{title}}\n",
        )
        .expect("write template");

        let note = create_note(&vault, "Standup", Some("meeting")).expect("create");
        assert_eq!(note.tags, vec!["meeting"]);
        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("Agenda for Standup"));
    }

    #[test]
    fn named_template_missing_is_an_error() {
        let (_guard, vault) = temp_vault();
        std::fs::create_dir_all(vault.layout().templates_dir()).expect("mkdir templates");
        let err = create_note(&vault, "X", Some("absent")).expect_err("missing template");
        assert!(matches!(
            err,
            crate::error::Error::Template(template::TemplateError::NotFound { .. })
        ));
    }

    #[test]
    fn today_note_creates_then_reuses() {
        let (_guard, vault) = temp_vault();
        std::fs::create_dir_all(vault.layout().templates_dir()).expect("templates");
        std::fs::write(vault.layout().today_template(), template::TODAY_TEMPLATE)
            .expect("seed today");

        let first = today_note(&vault).expect("first");
        assert!(first.created);
        assert_eq!(first.note.title, datetime::today_local_date());
        assert_eq!(first.note.tags, vec!["daily"]);

        // A second call reuses the same note rather than creating another.
        let second = today_note(&vault).expect("second");
        assert!(!second.created);
        assert_eq!(second.note.path, first.note.path);
    }

    #[test]
    fn today_note_requires_today_template() {
        let (_guard, vault) = temp_vault();
        let err = today_note(&vault).expect_err("missing today template");
        assert!(matches!(err, crate::error::Error::Template(_)));
    }

    #[test]
    fn untitled_fallback_for_empty_title() {
        let (_guard, vault) = temp_vault();
        let note = create_note(&vault, "???", None).expect("create");
        assert!(
            note.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-untitled.md")
        );
    }
}
