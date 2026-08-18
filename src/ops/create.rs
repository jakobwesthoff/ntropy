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
//!
//! [`create_empty_note`] is the same placement logic without the template step,
//! for callers that author the note's content themselves.

use crate::datetime;
use crate::error::Result;
use crate::fsutil;
use crate::id::Id;
use crate::note::{Note, filename};
use crate::scan;
use crate::session::VaultSession;
use crate::template::{self, TemplateVars};
use crate::text::slug;

/// Where a new note titled `title` goes: a fresh identity and the canonical path
/// it implies.
///
/// Both creation entry points route through this, so the one decision neither of
/// them may make differently — which file in `all-notes/` a new note is — has a
/// single home. The name follows the vault's storage: `<ulid>-<slug>.md`
/// plaintext, `<ulid>.age` encrypted.
fn place(session: &VaultSession, title: &str) -> (Id, std::path::PathBuf) {
    let id = Id::generate();
    let path = session.layout().all_notes().join(filename::build_for(
        &id,
        title,
        session.cipher().extension(),
    ));
    (id, path)
}

/// Create a note titled `title` in `vault` from a template.
///
/// `template` selects `<name>.md` from the vault's templates directory; `None`
/// uses `default.md` (falling back to the embedded default when absent). Returns
/// the parsed [`Note`], whose `path` is the file just written.
pub fn create_note(session: &VaultSession, title: &str, template: Option<&str>) -> Result<Note> {
    let (id, path) = place(session, title);
    let slug = slug::slugify(title);
    let date = datetime::render_local_date(id.timestamp_ms())?;

    let template = match template {
        None => template::load_or_default(&session.layout().default_template())?,
        Some(name) => template::load_named(&session.layout().templates_dir(), name)?,
    };
    let vars = TemplateVars {
        title: title.to_string(),
        id: id.to_string(),
        date,
        slug: slug.clone(),
    };
    let content = template::render(&template, &vars);

    // A malformed template (e.g. one missing a `title` field, or one that
    // embeds `{{title}}` into a bare plain scalar where a `: `-bearing title
    // breaks the line) renders to text `Note::parse` rejects. Parsing before
    // anything touches disk means a failed `new` leaves nothing behind for the
    // rest of the CLI to warn about on every later scan. `modified` is
    // unknown for content that only exists in memory, so it is filled in
    // below once the file has actually been stat'd.
    let mut note = Note::parse(path.clone(), &content, None)?;

    // The canonical store must exist before the atomic write places a temp file
    // beside the destination; creating it is idempotent on an initialized vault.
    //
    // Writing through the cipher is what lets this work on a locked vault:
    // encrypting needs only the public recipient, and nothing above has read a
    // note (ADR 0041).
    fsutil::create_dir_all(&session.layout().all_notes())?;
    session.cipher().write(&path, &content)?;

    note.modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    Ok(note)
}

/// Create an empty note titled `title` in `vault` and return its path.
///
/// Nothing but the canonical file is produced: no template is consulted and no
/// byte is written into it. The identity, the location and the filename are
/// still ntropy's to decide, which is the whole point — a caller that wants to
/// author the note itself gets the one part it cannot invent (ADR 0004) and
/// nothing else.
///
/// The file is not a well-formed note until the caller writes frontmatter into
/// it, so until then it is skipped with a warning like any other malformed file
/// (ADR 0019). Returns the path rather than a [`Note`] because there is no note
/// to parse yet.
pub fn create_empty_note(session: &VaultSession, title: &str) -> Result<std::path::PathBuf> {
    let (_id, path) = place(session, title);

    // Writing through the cipher keeps an encrypted vault's `<ulid>.age` file a
    // valid age container that decrypts to the empty string, rather than a
    // zero-byte file no reader could open. Encrypting needs only the public
    // recipient, so this works on a locked vault (ADR 0041).
    fsutil::create_dir_all(&session.layout().all_notes())?;
    session.cipher().write(&path, "")?;

    Ok(path)
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
pub fn today_note(session: &VaultSession) -> Result<TodayOutcome> {
    let date = datetime::today_local_date();

    // A vault that has not created any note yet has no `all-notes/`; treat that
    // as "no match" rather than scanning a missing directory.
    let all_notes = session.layout().all_notes();
    if all_notes.is_dir() {
        let scan = scan::scan_notes_dir(&all_notes, session.cipher())?;
        if let Some(existing) = scan.notes.into_iter().find(|n| n.title == date) {
            return Ok(TodayOutcome {
                note: existing,
                created: false,
            });
        }
    }

    let note = create_note(session, &date, Some("today"))?;
    Ok(TodayOutcome {
        note,
        created: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;

    /// Initialize just enough of a vault for `create_note` to run.
    fn temp_vault() -> (tempfile::TempDir, VaultSession) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join(".ntropy")).expect("mkdir .ntropy");
        let session = VaultSession::plaintext(Vault::new(dir.path()));
        (dir, session)
    }

    #[test]
    fn creates_file_with_canonical_name() {
        let (_guard, session) = temp_vault();
        let note = create_note(&session, "My First Note", None).expect("create");

        let name = note.path.file_name().unwrap().to_string_lossy();
        assert!(name.ends_with("-my-first-note.md"));
        assert_eq!(name.len(), 26 + "-my-first-note.md".len());
        assert!(note.path.exists());
    }

    #[test]
    fn rendered_content_round_trips_into_note() {
        let (_guard, session) = temp_vault();
        let note = create_note(&session, "My First Note", None).expect("create");
        assert_eq!(note.title, "My First Note");
        assert_eq!(note.tags, Vec::<String>::new());

        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("title: My First Note"));
        assert!(on_disk.contains("# My First Note"));
    }

    #[test]
    fn yaml_special_title_creates_a_well_formed_note() {
        // Reproduces the bug in
        // todos/01kwvczg18dprcrdja9dzzqzde-failed-new-leaves-malformed-note-file-in-all-notes.md:
        // a title containing `: ` used to break the default template's
        // `title: {{title}}` line. `create_note` now succeeds and the file it
        // writes is a well-formed note (ADR 0034).
        let (_guard, session) = temp_vault();
        let note = create_note(&session, "Q3: Planning kickoff", None).expect("create");
        assert_eq!(note.title, "Q3: Planning kickoff");

        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("title: 'Q3: Planning kickoff'"));
    }

    #[test]
    fn uses_custom_template_when_present() {
        let (_guard, session) = temp_vault();
        let templates = session.layout().templates_dir();
        std::fs::create_dir_all(&templates).expect("mkdir templates");
        std::fs::write(
            session.layout().default_template(),
            "---\ntitle: {{title}}\n---\nCustom body for {{slug}}\n",
        )
        .expect("write template");

        let note = create_note(&session, "Hello World", None).expect("create");
        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("Custom body for hello-world"));
    }

    #[test]
    fn uses_named_template_when_selected() {
        let (_guard, session) = temp_vault();
        let templates = session.layout().templates_dir();
        std::fs::create_dir_all(&templates).expect("mkdir templates");
        std::fs::write(
            templates.join("meeting.md"),
            "---\ntitle: {{title}}\ntags: [meeting]\n---\nAgenda for {{title}}\n",
        )
        .expect("write template");

        let note = create_note(&session, "Standup", Some("meeting")).expect("create");
        assert_eq!(note.tags, vec!["meeting"]);
        let on_disk = std::fs::read_to_string(&note.path).expect("read");
        assert!(on_disk.contains("Agenda for Standup"));
    }

    #[test]
    fn template_without_title_field_errors_and_leaves_no_stray_file() {
        // Reproduces aspect 1 of
        // todos/01kwvczg18dprcrdja9dzzqzde-failed-new-leaves-malformed-note-file-in-all-notes.md:
        // a custom template whose frontmatter has no `title` field renders to a
        // file `Note::parse` rejects. `create_note` must fail without writing
        // anything to `all-notes/`.
        let (_guard, session) = temp_vault();
        let templates = session.layout().templates_dir();
        std::fs::create_dir_all(&templates).expect("mkdir templates");
        std::fs::write(
            session.layout().default_template(),
            "---\ntags: []\n---\nBody with no title field.\n",
        )
        .expect("write template");

        let err = create_note(&session, "Hello World", None).expect_err("invalid note");
        assert!(matches!(
            err,
            crate::error::Error::Note(crate::note::NoteError::Frontmatter(_))
        ));

        let all_notes = session.layout().all_notes();
        let stray = if all_notes.is_dir() {
            std::fs::read_dir(&all_notes)
                .expect("read all-notes")
                .count()
        } else {
            0
        };
        assert_eq!(stray, 0, "a failed create must not strand any file");
    }

    #[test]
    fn embedded_placeholder_breaking_yaml_errors_and_leaves_no_stray_file() {
        // ADR 0034 substitutes `{{title}}` verbatim when it appears embedded in
        // a bare plain scalar (e.g. `title: Meeting {{title}} notes`), rather
        // than quoting the whole line as it does for a placeholder that is the
        // entire value. A title containing `: ` still breaks the YAML in that
        // case, so this pins the validate-before-write safety net that catches
        // exactly that row.
        let (_guard, session) = temp_vault();
        let templates = session.layout().templates_dir();
        std::fs::create_dir_all(&templates).expect("mkdir templates");
        std::fs::write(
            session.layout().default_template(),
            "---\ntitle: Meeting {{title}} notes\ntags: []\n---\nBody.\n",
        )
        .expect("write template");

        let err =
            create_note(&session, "Q3: Planning kickoff", None).expect_err("invalid yaml note");
        assert!(matches!(err, crate::error::Error::Note(_)));

        let all_notes = session.layout().all_notes();
        let stray = if all_notes.is_dir() {
            std::fs::read_dir(&all_notes)
                .expect("read all-notes")
                .count()
        } else {
            0
        };
        assert_eq!(stray, 0, "a failed create must not strand any file");
    }

    #[test]
    fn named_template_missing_is_an_error() {
        let (_guard, session) = temp_vault();
        std::fs::create_dir_all(session.layout().templates_dir()).expect("mkdir templates");
        let err = create_note(&session, "X", Some("absent")).expect_err("missing template");
        assert!(matches!(
            err,
            crate::error::Error::Template(template::TemplateError::NotFound { .. })
        ));
    }

    #[test]
    fn empty_note_gets_the_canonical_name_and_no_content() {
        let (_guard, session) = temp_vault();
        let path = create_empty_note(&session, "My First Note").expect("create");

        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.ends_with("-my-first-note.md"));
        assert_eq!(name.len(), 26 + "-my-first-note.md".len());
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    }

    #[test]
    fn empty_note_consults_no_template() {
        // The point of the empty mode: even a `default.md` that could never
        // render into a well-formed note is irrelevant, because it is never
        // read.
        let (_guard, session) = temp_vault();
        std::fs::create_dir_all(session.layout().templates_dir()).expect("mkdir templates");
        std::fs::write(
            session.layout().default_template(),
            "no frontmatter at all\n",
        )
        .expect("write template");

        let path = create_empty_note(&session, "Unaffected").expect("create");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    }

    #[test]
    fn empty_note_is_skipped_by_a_scan_until_it_is_filled() {
        // An empty file has no frontmatter, so it is malformed by the ordinary
        // rules (ADR 0019) rather than a special case: warned about and left out
        // of results until content arrives.
        let (_guard, session) = temp_vault();
        let path = create_empty_note(&session, "Filled Later").expect("create");

        let before = crate::ops::search(&session, None).expect("search");
        assert!(before.notes.is_empty(), "an empty file is not yet a note");
        assert_eq!(before.warnings.len(), 1);

        std::fs::write(
            &path,
            "---\ntitle: Filled Later\ntags: [done]\n---\n# Filled Later\n",
        )
        .expect("fill");

        let after = crate::ops::search(&session, None).expect("search");
        assert!(after.warnings.is_empty());
        assert_eq!(after.notes.len(), 1);
        assert_eq!(after.notes[0].title, "Filled Later");
        assert_eq!(after.notes[0].tags, vec!["done"]);
        assert_eq!(after.notes[0].path, path);
    }

    #[test]
    fn empty_note_identity_survives_being_filled_in() {
        // The identity the caller cannot invent is handed over in the filename
        // and is unchanged by whatever the caller writes into the file.
        let (_guard, session) = temp_vault();
        let path = create_empty_note(&session, "Identity Holder").expect("create");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();

        std::fs::write(
            &path,
            "---\ntitle: Identity Holder\n---\n# Identity Holder\n",
        )
        .expect("fill");
        let matches = crate::ops::search(&session, None).expect("search");
        assert_eq!(matches.notes[0].id.to_string(), name[..26]);
    }

    #[test]
    fn empty_note_falls_back_to_untitled() {
        let (_guard, session) = temp_vault();
        let path = create_empty_note(&session, "???").expect("create");
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-untitled.md")
        );
    }

    #[test]
    fn today_note_creates_then_reuses() {
        let (_guard, session) = temp_vault();
        std::fs::create_dir_all(session.layout().templates_dir()).expect("templates");
        std::fs::write(
            session.layout().today_template(),
            crate::vault::seed::TODAY_TEMPLATE,
        )
        .expect("seed today");

        let first = today_note(&session).expect("first");
        assert!(first.created);
        assert_eq!(first.note.title, datetime::today_local_date());
        assert_eq!(first.note.tags, vec!["daily"]);

        // A second call reuses the same note rather than creating another.
        let second = today_note(&session).expect("second");
        assert!(!second.created);
        assert_eq!(second.note.path, first.note.path);
    }

    #[test]
    fn today_note_requires_today_template() {
        let (_guard, session) = temp_vault();
        let err = today_note(&session).expect_err("missing today template");
        assert!(matches!(err, crate::error::Error::Template(_)));
    }

    #[test]
    fn untitled_fallback_for_empty_title() {
        let (_guard, session) = temp_vault();
        let note = create_note(&session, "???", None).expect("create");
        assert!(
            note.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-untitled.md")
        );
    }

    /// Creating notes in a vault whose notes are encrypted.
    #[cfg(feature = "encryption")]
    mod encrypted {
        use crate::ops::create::{create_empty_note, create_note};
        use crate::test_support::encrypted::encrypted_vault;

        #[test]
        fn an_empty_note_is_named_by_its_identity_alone() {
            let fixture = encrypted_vault();
            let path = create_empty_note(&fixture.session, "Quarterly Review").expect("create");
            let name = path
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            assert!(name.ends_with(".age"));
            assert!(
                !name.contains("quarterly"),
                "nothing title-derived may reach the filename: {name}"
            );
        }

        #[test]
        fn an_empty_note_is_a_readable_container_holding_nothing() {
            // Not a zero-byte file: the cipher writes a real age container, so
            // the vault's own reader can open it like any other note.
            let fixture = encrypted_vault();
            let path = create_empty_note(&fixture.session, "Nothing Inside").expect("create");
            assert_ne!(std::fs::metadata(&path).expect("stat").len(), 0);
            assert_eq!(fixture.session.cipher().read(&path).expect("read"), "");
        }

        #[test]
        fn creating_an_empty_note_works_on_a_locked_vault() {
            // Same property as the templated path: encrypting needs only the
            // public recipient.
            let fixture = encrypted_vault();
            let locked = fixture.locked();
            assert!(!locked.is_unlocked());

            let path = create_empty_note(&locked, "Written While Locked").expect("create");
            assert!(path.is_file());
            assert_eq!(fixture.session.cipher().read(&path).expect("read"), "");
        }

        #[test]
        fn a_new_note_is_named_by_its_identity_alone() {
            let fixture = encrypted_vault();
            let note = create_note(&fixture.session, "Quarterly Review", None).expect("create");
            let name = note
                .path
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            assert_eq!(name, format!("{}.age", note.id));
            assert!(
                !name.contains("quarterly"),
                "nothing title-derived may reach the filename: {name}"
            );
        }

        #[test]
        fn creation_works_on_a_locked_vault() {
            // The headline property of the asymmetric key model: encrypting a
            // new note needs only the public recipient.
            let fixture = encrypted_vault();
            let locked = fixture.locked();
            assert!(!locked.is_unlocked());

            let note = create_note(&locked, "Written While Locked", None).expect("create");
            assert!(note.path.is_file());

            // And it really is the note, once a key shows up.
            let content = fixture.session.cipher().read(&note.path).expect("read");
            assert!(content.contains("Written While Locked"), "got: {content}");
        }

        #[test]
        fn the_file_on_disk_is_not_readable_as_the_note() {
            let fixture = encrypted_vault();
            let note = create_note(&fixture.session, "Secret Plans", None).expect("create");
            let raw = std::fs::read(&note.path).expect("read bytes");
            assert!(
                !raw.windows(12).any(|w| w == b"Secret Plans"),
                "the title reached the file in the clear"
            );
        }

        #[test]
        fn a_created_note_reads_back_through_a_scan() {
            let fixture = encrypted_vault();
            create_note(&fixture.session, "Round Trip", None).expect("create");
            let matches = crate::ops::search(&fixture.session, None).expect("search");
            assert_eq!(matches.notes.len(), 1);
            assert_eq!(matches.notes[0].title, "Round Trip");
        }

        #[test]
        fn a_malformed_template_leaves_no_stray_file() {
            // The ADR 0034 safety net, in an encrypted vault: parsing happens
            // before anything touches disk either way.
            let fixture = encrypted_vault();
            let templates = fixture.session.layout().templates_dir();
            std::fs::create_dir_all(&templates).expect("templates dir");
            std::fs::write(templates.join("broken.md"), "no frontmatter at all\n")
                .expect("write template");

            assert!(create_note(&fixture.session, "Doomed", Some("broken")).is_err());
            let entries: Vec<_> = std::fs::read_dir(fixture.session.layout().all_notes())
                .expect("read dir")
                .collect();
            assert!(entries.is_empty(), "a failed create left a file behind");
        }
    }
}
