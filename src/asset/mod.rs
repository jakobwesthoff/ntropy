// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A note's assets: where they live, whose they are and how a link into them
//! is read.
//!
//! A note keeps its files in `all-notes/_assets/<stem>/`, where `<stem>` is
//! the note's file name without its extension, so the folder is readable
//! without ntropy. The folder is owned by the note whose ULID leads its name,
//! whatever slug follows: ownership goes by identity, the way a note link
//! resolves by its ULID (ADR 0028), so a folder whose slug has gone stale
//! between a retitle and the next `reconcile` still belongs to its note.
//!
//! This module is pure: it classifies paths and names. Reading folders from
//! disk, and everything that changes them, builds on it.

mod check;
mod files;
mod links;
mod size;
mod sync;

pub use check::{AssetIssue, check};
pub use files::{
    AddOptions, AddReport, Added, AssetError, BrokenLink, Linked, ListedFile, Listing, add,
    folder_of, is_image_name, list,
};
pub use links::{
    LocalLink, Reference, html_ranges, local_links, names_a_file, references, relink_folder,
};
pub use size::{format_size, parse_size};
pub(crate) use sync::is_system_metadata;
pub use sync::{
    AssetSync, FolderMove, RefusalReason, RefusedMove, Relink, first_unencryptable, plan_renames,
    read_folders, stale_names, sync,
};

use crate::id::{Id, ULID_LEN};
use crate::render::markdown::resolve_root_relative;
use crate::vault::layout::{ALL_NOTES_DIR, ASSETS_DIR};

/// Where a link's target lies, resolved textually against the note's
/// directory the way every renderer resolves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// Inside the vault's `all-notes/_assets/`. `folder` is the first
    /// component below it, empty when the target is `_assets` itself, and
    /// `rest` is the remaining path.
    Managed { folder: String, rest: String },
    /// Inside an `_assets` directory somewhere else, such as the one every
    /// view directory links to. `folder` is the component after it.
    Elsewhere { folder: String },
    /// Any other path inside the vault.
    Unmanaged,
    /// A path that climbs out of the vault.
    Escapes,
}

/// What a link's place means for the note that holds the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Standing {
    /// A file in the note's own asset folder.
    Own,
    /// A file in another note's asset folder, or in `_assets/` outside a
    /// folder of any note: a note has only its own assets.
    Foreign,
    /// A file outside every asset folder.
    Unmanaged,
    /// A path that climbs out of the vault.
    Escapes,
}

/// Classify `dest`, a link target written in a note whose directory is
/// `note_dir` (root-absolute, `/all-notes` for a standard layout).
///
/// A fragment is cut off first, as the renderers do. The `_assets` and
/// `all-notes` components match ignoring ASCII case, so a case-insensitive
/// filesystem cannot be used to name a folder the exact spelling would not.
pub fn place(note_dir: &str, dest: &str) -> Place {
    let path = dest.split_once('#').map_or(dest, |(path, _)| path);
    let Some(resolved) = resolve_root_relative(note_dir, path) else {
        return Place::Escapes;
    };
    let parts: Vec<&str> = resolved
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let Some(at) = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case(ASSETS_DIR))
    else {
        return Place::Unmanaged;
    };
    let below = &parts[at + 1..];
    let in_notes_dir = at == 1 && parts[0].eq_ignore_ascii_case(ALL_NOTES_DIR);
    match (in_notes_dir, below.split_first()) {
        (true, Some((folder, rest))) => Place::Managed {
            folder: folder.to_string(),
            rest: rest.join("/"),
        },
        (true, None) => Place::Managed {
            folder: String::new(),
            rest: String::new(),
        },
        (false, Some((folder, _))) => Place::Elsewhere {
            folder: folder.to_string(),
        },
        (false, None) => Place::Unmanaged,
    }
}

impl Place {
    /// What this place means for the note with identity `note`.
    pub fn standing(&self, note: Id) -> Standing {
        match self {
            Place::Managed { folder, .. } if owner(folder) == Some(note) => Standing::Own,
            Place::Managed { .. } => Standing::Foreign,
            // Outside the managed root only a folder some other note owns
            // makes the link foreign: a user may keep an `_assets` directory
            // of their own anywhere else in the vault, and the notes' own
            // folder reached through a view link is no managed path either.
            Place::Elsewhere { folder } => match owner(folder) {
                Some(owner) if owner != note => Standing::Foreign,
                _ => Standing::Unmanaged,
            },
            Place::Unmanaged => Standing::Unmanaged,
            Place::Escapes => Standing::Escapes,
        }
    }
}

/// The note that owns the asset folder called `folder`: the identity in its
/// first 26 characters, when they are the whole name or are followed by `-`.
pub fn owner(folder: &str) -> Option<Id> {
    let prefix = folder.get(..ULID_LEN)?;
    let rest = &folder[ULID_LEN..];
    if !(rest.is_empty() || rest.starts_with('-')) {
        return None;
    }
    prefix.parse().ok()
}

/// The folder a note's assets go in, chosen among the names that exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderChoice {
    /// The one existing folder the note owns, under its present name.
    Existing(String),
    /// The note has none yet: create this one.
    Create(String),
    /// The note owns several; the commands refuse to pick.
    Several(Vec<String>),
}

/// Choose the folder of the note `id`, whose file stem is `stem`, among
/// `listed`, the names found in `_assets/`.
///
/// An existing folder is used whatever its slug. Between a retitle and the
/// next `reconcile` it carries the old name, and creating one under the new
/// name would leave the note with two.
pub fn choose_folder(listed: &[String], id: Id, stem: &str) -> FolderChoice {
    let mut owned: Vec<String> = listed
        .iter()
        .filter(|name| owner(name) == Some(id))
        .cloned()
        .collect();
    owned.sort();
    match owned.len() {
        0 => FolderChoice::Create(stem.to_string()),
        1 => FolderChoice::Existing(owned.remove(0)),
        _ => FolderChoice::Several(owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn id(text: &str) -> Id {
        text.parse().expect("a valid ULID")
    }

    fn managed(folder: &str, rest: &str) -> Place {
        Place::Managed {
            folder: folder.to_string(),
            rest: rest.to_string(),
        }
    }

    #[test]
    fn a_relative_target_below_assets_is_managed() {
        assert_eq!(
            place("/all-notes", "_assets/folder/chart.png"),
            managed("folder", "chart.png")
        );
    }

    #[test]
    fn a_target_through_the_vault_root_is_managed() {
        assert_eq!(
            place("/all-notes", "../all-notes/_assets/folder/a/b.png"),
            managed("folder", "a/b.png")
        );
    }

    #[test]
    fn a_root_absolute_target_is_managed() {
        assert_eq!(
            place("/all-notes", "/all-notes/_assets/folder/chart.png"),
            managed("folder", "chart.png")
        );
    }

    #[test]
    fn the_assets_and_notes_components_match_ignoring_case() {
        assert_eq!(
            place("/all-notes", "/All-Notes/_ASSETS/folder/chart.png"),
            managed("folder", "chart.png")
        );
    }

    #[test]
    fn a_fragment_is_not_part_of_the_path() {
        assert_eq!(
            place("/all-notes", "_assets/folder/doc.pdf#page=2"),
            managed("folder", "doc.pdf")
        );
    }

    #[test]
    fn a_file_directly_in_assets_has_itself_as_its_folder() {
        assert_eq!(
            place("/all-notes", "_assets/loose.png"),
            managed("loose.png", "")
        );
    }

    #[test]
    fn assets_itself_has_no_folder() {
        assert_eq!(place("/all-notes", "_assets/"), managed("", ""));
    }

    #[test]
    fn an_assets_directory_elsewhere_is_named_with_its_next_component() {
        assert_eq!(
            place("/all-notes", "../by-tag/_assets/folder/chart.png"),
            Place::Elsewhere {
                folder: "folder".to_string()
            }
        );
    }

    #[test]
    fn a_vault_root_assets_directory_is_unmanaged() {
        assert_eq!(place("/all-notes", "../assets/logo.svg"), Place::Unmanaged);
    }

    #[test]
    fn an_assets_directory_elsewhere_without_a_folder_is_unmanaged() {
        assert_eq!(place("/all-notes", "../projects/_assets"), Place::Unmanaged);
    }

    #[test]
    fn a_file_beside_the_notes_is_unmanaged() {
        assert_eq!(place("/all-notes", "transcript.txt"), Place::Unmanaged);
    }

    #[test]
    fn a_target_that_climbs_out_of_the_vault_escapes() {
        assert_eq!(place("/all-notes", "../../outside.png"), Place::Escapes);
    }

    #[test]
    fn an_exact_note_id_owns_its_folder() {
        assert_eq!(owner(ULID), Some(id(ULID)));
    }

    #[test]
    fn a_slug_after_the_id_does_not_change_the_owner() {
        assert_eq!(owner(&format!("{ULID}-q3-planning")), Some(id(ULID)));
    }

    #[test]
    fn a_lowercase_id_owns_too() {
        assert_eq!(owner(&ULID.to_lowercase()), Some(id(ULID)));
    }

    #[test]
    fn a_name_that_is_not_an_id_has_no_owner() {
        assert_eq!(owner("shared"), None);
        assert_eq!(owner(""), None);
        assert_eq!(owner(&ULID[..25]), None);
    }

    #[test]
    fn an_id_glued_to_more_characters_has_no_owner() {
        assert_eq!(owner(&format!("{ULID}x")), None);
    }

    #[test]
    fn the_notes_own_folder_is_its_own() {
        let place = managed(&format!("{ULID}-note"), "chart.png");
        assert_eq!(place.standing(id(ULID)), Standing::Own);
    }

    #[test]
    fn another_notes_folder_is_foreign() {
        let place = managed(&format!("{OTHER}-note"), "chart.png");
        assert_eq!(place.standing(id(ULID)), Standing::Foreign);
    }

    #[test]
    fn a_folder_without_an_owner_is_foreign() {
        assert_eq!(
            managed("shared", "chart.png").standing(id(ULID)),
            Standing::Foreign
        );
    }

    #[test]
    fn a_file_loose_in_assets_is_foreign() {
        assert_eq!(
            managed("loose.png", "").standing(id(ULID)),
            Standing::Foreign
        );
    }

    #[test]
    fn another_notes_folder_reached_through_a_view_is_foreign() {
        let place = Place::Elsewhere {
            folder: format!("{OTHER}-note"),
        };
        assert_eq!(place.standing(id(ULID)), Standing::Foreign);
    }

    #[test]
    fn an_assets_directory_a_user_keeps_elsewhere_is_unmanaged() {
        let place = Place::Elsewhere {
            folder: "logo.png".to_string(),
        };
        assert_eq!(place.standing(id(ULID)), Standing::Unmanaged);
    }

    #[test]
    fn the_notes_own_folder_reached_through_a_view_is_unmanaged() {
        let place = Place::Elsewhere {
            folder: format!("{ULID}-note"),
        };
        assert_eq!(place.standing(id(ULID)), Standing::Unmanaged);
    }

    #[test]
    fn unmanaged_and_escaping_places_keep_their_standing() {
        assert_eq!(Place::Unmanaged.standing(id(ULID)), Standing::Unmanaged);
        assert_eq!(Place::Escapes.standing(id(ULID)), Standing::Escapes);
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn a_note_with_no_folder_gets_one_named_like_its_file() {
        let listed = names(&[&format!("{OTHER}-other")]);
        assert_eq!(
            choose_folder(&listed, id(ULID), &format!("{ULID}-note")),
            FolderChoice::Create(format!("{ULID}-note"))
        );
    }

    #[test]
    fn a_stale_named_folder_is_still_the_notes_folder() {
        let listed = names(&[&format!("{ULID}-old-title")]);
        assert_eq!(
            choose_folder(&listed, id(ULID), &format!("{ULID}-new-title")),
            FolderChoice::Existing(format!("{ULID}-old-title"))
        );
    }

    #[test]
    fn two_folders_of_one_note_are_not_chosen_between() {
        let listed = names(&[&format!("{ULID}-b"), &format!("{ULID}-a")]);
        assert_eq!(
            choose_folder(&listed, id(ULID), &format!("{ULID}-c")),
            FolderChoice::Several(names(&[&format!("{ULID}-a"), &format!("{ULID}-b")]))
        );
    }
}
