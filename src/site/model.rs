// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The site's structure, computed from the exported notes and the vault's
//! views before any page is rendered (ADR 0054).
//!
//! The model answers every structural question the pages ask: what each
//! note's page is called, which sections the sidebar has and how their
//! groups nest, which notes a group holds, where a note sits first (its
//! breadcrumb, its previous and next neighbours), and what the front page
//! is. It is pure data over the note set, so the page paths and the
//! navigation are testable without rendering a byte of HTML.

use std::collections::BTreeMap;

use crate::id::Id;
use crate::note::Note;
use crate::text::slug;
use crate::view::{self, ViewDef};

use super::SiteOptions;

/// The number of newest notes the generated front page lists.
pub const FRONT_PAGE_RECENT: usize = 10;

/// The most related notes a note page lists.
pub const RELATED_LIMIT: usize = 8;

/// The frontmatter field whose view the tag section already is.
const TAGS_FIELD: &str = "tags";

/// One exported note, as the pages refer to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEntry {
    pub id: Id,
    pub title: String,
    pub created: String,
    pub tags: Vec<String>,
    /// The page path within the site: `notes/<slug>.html`, with a ULID tail
    /// on the slug where two notes share one.
    pub page: String,
}

/// A group inside a section: one grouping value (a tag, a field value) and
/// the notes and child groups under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The full value, `programming/rust`.
    pub value: String,
    /// The last segment, what the sidebar shows.
    pub label: String,
    /// Page path: `<section root>/<value>/index.html`.
    pub page: String,
    /// The notes whose value is exactly this group's, newest first, as
    /// indices into the model's notes.
    pub notes: Vec<usize>,
    /// Child groups, by label.
    pub children: Vec<Group>,
}

impl Group {
    /// The notes of this group and of every group below it, newest first,
    /// each once: what the group's own page lists (ADR 0054, tag pages).
    pub fn descendants(&self, model: &Model) -> Vec<usize> {
        let mut all = Vec::new();
        self.collect(&mut all);
        all.sort_by(|a, b| model.notes[*b].id.cmp(&model.notes[*a].id));
        all.dedup();
        all
    }

    fn collect(&self, into: &mut Vec<usize>) {
        into.extend(self.notes.iter().copied());
        for child in &self.children {
            child.collect(into);
        }
    }
}

/// What a section groups by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    /// A configured view over a frontmatter field.
    View { field: String },
    /// The tag hierarchy.
    Tags,
}

/// One sidebar section: a configured view, or the tag tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub kind: SectionKind,
    /// The sidebar heading: the view's name, or `tags`.
    pub title: String,
    /// Page path: `views/<name>/index.html` or `tags/index.html`.
    pub page: String,
    /// The section's root directory in the site, without trailing slash.
    pub root: String,
    pub groups: Vec<Group>,
}

/// Where a note sits first in sidebar order, which fixes its breadcrumb and
/// its previous and next neighbours (ADR 0054).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub section: usize,
    /// The path of child indices from the section's groups down to the
    /// group holding the note.
    pub path: Vec<usize>,
    pub prev: Option<usize>,
    pub next: Option<usize>,
}

/// The front page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Front {
    /// The configured index note, by model index.
    Note(usize),
    /// A generated overview.
    Overview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub title: String,
    pub lang: String,
    /// Every exported note, newest first.
    pub notes: Vec<NoteEntry>,
    pub sections: Vec<Section>,
    /// Per note, its first placement, or `None` for a note in no group.
    pub placements: Vec<Option<Placement>>,
    pub front: Front,
    /// Non-fatal findings while building: a configured index note that is
    /// not in the exported set.
    pub warnings: Vec<String>,
}

impl Model {
    /// Build the model. `notes` is the exported set in any order; the model
    /// orders it newest first. `fallback_title` is the site title when the
    /// options configure none.
    pub fn build(
        notes: &[Note],
        views: &[ViewDef],
        options: &SiteOptions,
        fallback_title: &str,
    ) -> Result<Model, crate::error::Error> {
        let mut ordered: Vec<&Note> = notes.iter().collect();
        ordered.sort_by_key(|note| std::cmp::Reverse(note.id));

        // Page names: the slug, disambiguated by a ULID tail among the notes
        // that share one, the rule the view leaves use.
        let bases: Vec<(Id, String)> = ordered
            .iter()
            .map(|note| (note.id, slug::slugify(&note.title)))
            .collect();
        let names = view::leaf::disambiguate(&bases);

        let mut entries = Vec::with_capacity(ordered.len());
        for (note, name) in ordered.iter().zip(names) {
            entries.push(NoteEntry {
                id: note.id,
                title: note.title.clone(),
                created: note.created_date()?,
                tags: note.tags.clone(),
                page: format!("notes/{name}.html"),
            });
        }

        // Sections in sidebar order: the configured views, then the tags. A
        // view over the tags field would repeat the tag section group for
        // group, so it is no section and gets no pages.
        let mut sections = Vec::new();
        for view in views.iter().filter(|view| view.field != TAGS_FIELD) {
            let root = format!("views/{}", view.name);
            let values: Vec<Vec<String>> = ordered
                .iter()
                .map(|note| view::group_values(note, &view.field))
                .collect();
            sections.push(Section {
                kind: SectionKind::View {
                    field: view.field.clone(),
                },
                title: view.name.clone(),
                page: format!("{root}/index.html"),
                groups: tree(&root, &values),
                root,
            });
        }
        let tag_values: Vec<Vec<String>> = ordered.iter().map(|note| note.tags.clone()).collect();
        sections.push(Section {
            kind: SectionKind::Tags,
            title: "tags".to_string(),
            page: "tags/index.html".to_string(),
            groups: tree("tags", &tag_values),
            root: "tags".to_string(),
        });

        let placements = placements(&entries, &sections);

        let mut warnings = Vec::new();
        let front = match &options.index {
            Some(id) => match entries.iter().position(|entry| entry.id.to_string() == *id) {
                Some(index) => Front::Note(index),
                None => {
                    warnings.push(format!(
                        "the configured index note {id} is not among the exported notes; the front page is a generated overview"
                    ));
                    Front::Overview
                }
            },
            None => Front::Overview,
        };

        Ok(Model {
            title: options
                .title
                .clone()
                .unwrap_or_else(|| fallback_title.to_string()),
            lang: options.lang().to_string(),
            notes: entries,
            sections,
            placements,
            front,
            warnings,
        })
    }

    /// The notes most related to the note at `index`, by the number of tags
    /// they share with it, ancestors counted: `a/b` shares `a` with `a/c`.
    /// Notes sharing nothing are left out; ties keep the newest first. At
    /// most [`RELATED_LIMIT`] notes.
    pub fn related(&self, index: usize) -> Vec<usize> {
        let own = tag_paths(&self.notes[index].tags);
        if own.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(usize, usize)> = self
            .notes
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(other, note)| {
                let shared = tag_paths(&note.tags)
                    .iter()
                    .filter(|path| own.contains(path.as_str()))
                    .count();
                (shared, other)
            })
            .filter(|(shared, _)| *shared > 0)
            .collect();
        scored.sort_by_key(|&(shared, other)| (std::cmp::Reverse(shared), other));
        scored
            .into_iter()
            .take(RELATED_LIMIT)
            .map(|(_, other)| other)
            .collect()
    }

    /// The page path of the note with `id`, if it is exported.
    pub fn page_of(&self, id: Id) -> Option<&str> {
        self.notes
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.page.as_str())
    }

    /// The group a placement points at.
    pub fn group_at(&self, placement: &Placement) -> &Group {
        let section = &self.sections[placement.section];
        let mut group = &section.groups[placement.path[0]];
        for &index in &placement.path[1..] {
            group = &group.children[index];
        }
        group
    }

    /// The groups from the section's root down to the placement's group,
    /// outermost first: the breadcrumb.
    pub fn trail(&self, placement: &Placement) -> Vec<&Group> {
        let section = &self.sections[placement.section];
        let mut trail = Vec::new();
        let mut group = &section.groups[placement.path[0]];
        trail.push(group);
        for &index in &placement.path[1..] {
            group = &group.children[index];
            trail.push(group);
        }
        trail
    }
}

/// Every tag of `tags` and every ancestor of it, each once: `a/b/c` yields
/// `a`, `a/b`, and `a/b/c`.
fn tag_paths(tags: &[String]) -> std::collections::BTreeSet<String> {
    let mut paths = std::collections::BTreeSet::new();
    for tag in tags {
        let mut path = String::new();
        for segment in tag.split('/').filter(|s| !s.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(segment);
            paths.insert(path.clone());
        }
    }
    paths
}

/// Build a section's group tree from each note's grouping values. Every
/// `/`-separated prefix of a value becomes a group, so `a/b/c` yields `a`,
/// `a/b`, and `a/b/c` nested; a note is a member of exactly the group its
/// value names. Children sort by label; members keep the notes' order,
/// which is newest first.
fn tree(root: &str, values_per_note: &[Vec<String>]) -> Vec<Group> {
    #[derive(Default)]
    struct Node {
        notes: Vec<usize>,
        children: BTreeMap<String, Node>,
    }

    let mut top: BTreeMap<String, Node> = BTreeMap::new();
    for (index, values) in values_per_note.iter().enumerate() {
        for value in values {
            insert(&mut top, value, index);
        }
    }

    fn insert(top: &mut BTreeMap<String, Node>, value: &str, index: usize) {
        let segments: Vec<&str> = value.split('/').filter(|s| !s.is_empty()).collect();
        let mut level = top;
        for (depth, segment) in segments.iter().enumerate() {
            let node = level.entry((*segment).to_string()).or_default();
            if depth + 1 == segments.len() {
                if !node.notes.contains(&index) {
                    node.notes.push(index);
                }
                return;
            }
            level = &mut node.children;
        }
    }

    fn convert(root: &str, prefix: &str, nodes: BTreeMap<String, Node>) -> Vec<Group> {
        nodes
            .into_iter()
            .map(|(label, node)| {
                let value = if prefix.is_empty() {
                    label.clone()
                } else {
                    format!("{prefix}/{label}")
                };
                Group {
                    page: format!("{root}/{value}/index.html"),
                    children: convert(root, &value, node.children),
                    notes: node.notes,
                    label,
                    value,
                }
            })
            .collect()
    }

    convert(root, "", top)
}

/// Each note's first placement in sidebar order: sections in order, groups
/// depth-first in their order, and within a group its members newest first.
/// Previous and next are the neighbours within that group.
fn placements(notes: &[NoteEntry], sections: &[Section]) -> Vec<Option<Placement>> {
    let mut placements: Vec<Option<Placement>> = vec![None; notes.len()];

    fn visit(
        groups: &[Group],
        section: usize,
        path: &mut Vec<usize>,
        placements: &mut Vec<Option<Placement>>,
    ) {
        for (index, group) in groups.iter().enumerate() {
            path.push(index);
            for (position, &note) in group.notes.iter().enumerate() {
                if placements[note].is_none() {
                    placements[note] = Some(Placement {
                        section,
                        path: path.clone(),
                        prev: position.checked_sub(1).map(|p| group.notes[p]),
                        next: group.notes.get(position + 1).copied(),
                    });
                }
            }
            visit(&group.children, section, path, placements);
            path.pop();
        }
    }

    for (index, section) in sections.iter().enumerate() {
        visit(&section.groups, index, &mut Vec::new(), &mut placements);
    }
    placements
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    const A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";
    const C: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";
    /// Shares A's date and differs only in its tail, to force a slug clash.
    const A2: &str = "01ARZ3NDEKTSV4RRFFQ69G5FBW";

    fn note(id: &str, title: &str, frontmatter: &str) -> Note {
        let content = format!("---\ntitle: {title}\n{frontmatter}---\nBody.\n");
        Note::parse(
            PathBuf::from(format!("/vault/all-notes/{id}-{}.md", slug::slugify(title))),
            &content,
            None,
        )
        .expect("the fixture note parses")
    }

    fn model(notes: &[Note], views: &[ViewDef], options: SiteOptions) -> Model {
        Model::build(notes, views, &options, "Vault").expect("the model builds")
    }

    fn titles<'a>(model: &'a Model, indices: &[usize]) -> Vec<&'a str> {
        indices
            .iter()
            .map(|&i| model.notes[i].title.as_str())
            .collect()
    }

    #[test]
    fn notes_are_ordered_newest_first_and_named_by_slug() {
        let notes = [
            note(A, "Alpha", ""),
            note(C, "Gamma", ""),
            note(B, "Beta", ""),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let pages: Vec<&str> = model.notes.iter().map(|n| n.page.as_str()).collect();
        assert_eq!(
            pages,
            ["notes/gamma.html", "notes/beta.html", "notes/alpha.html"]
        );
        assert_eq!(model.title, "Vault");
        assert_eq!(model.lang, "en");
    }

    #[test]
    fn equal_slugs_get_ulid_tails_and_only_those() {
        let notes = [
            note(A, "Review", ""),
            note(A2, "Review", ""),
            note(B, "Other", ""),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let pages: Vec<&str> = model.notes.iter().map(|n| n.page.as_str()).collect();
        assert_eq!(
            pages,
            [
                "notes/other.html",
                "notes/review-FBW.html",
                "notes/review-FAV.html"
            ]
        );
    }

    #[test]
    fn tags_form_a_nested_section_with_exact_membership() {
        let notes = [
            note(A, "Rust", "tags: [programming/rust]\n"),
            note(B, "Prog", "tags: [programming]\n"),
            note(C, "Both", "tags: [programming/rust, demo]\n"),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let tags = &model.sections[0];
        assert_eq!(tags.kind, SectionKind::Tags);
        assert_eq!(tags.page, "tags/index.html");
        let labels: Vec<&str> = tags.groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["demo", "programming"]);

        let programming = &tags.groups[1];
        assert_eq!(programming.page, "tags/programming/index.html");
        assert_eq!(titles(&model, &programming.notes), ["Prog"]);
        let rust = &programming.children[0];
        assert_eq!(rust.value, "programming/rust");
        assert_eq!(rust.page, "tags/programming/rust/index.html");
        assert_eq!(titles(&model, &rust.notes), ["Both", "Rust"]);
        // The group page lists descendants too, newest first.
        assert_eq!(
            titles(&model, &programming.descendants(&model)),
            ["Both", "Prog", "Rust"]
        );
    }

    #[test]
    fn views_come_before_tags_and_group_by_their_field() {
        let notes = [
            note(A, "Draft", "status: draft\n"),
            note(B, "Done", "status: Done\n"),
            note(C, "None", ""),
        ];
        let views = [ViewDef::new("by-status", "status")];
        let model = model(&notes, &views, SiteOptions::default());
        assert_eq!(model.sections.len(), 2);
        let status = &model.sections[0];
        assert_eq!(status.title, "by-status");
        assert_eq!(status.page, "views/by-status/index.html");
        assert_eq!(
            status.kind,
            SectionKind::View {
                field: "status".to_string()
            }
        );
        let labels: Vec<&str> = status.groups.iter().map(|g| g.label.as_str()).collect();
        // Values are normalized like tags: `Done` lowercases.
        assert_eq!(labels, ["done", "draft"]);
        assert_eq!(status.groups[0].page, "views/by-status/done/index.html");
        assert_eq!(model.sections[1].kind, SectionKind::Tags);
    }

    #[test]
    fn a_view_over_the_tags_field_is_no_section() {
        let notes = [note(A, "Tagged", "tags: [x]\nstatus: open\n")];
        let views = [
            ViewDef::new("by-tag", "tags"),
            ViewDef::new("by-status", "status"),
        ];
        let model = model(&notes, &views, SiteOptions::default());
        let titles: Vec<&str> = model.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["by-status", "tags"]);
    }

    #[test]
    fn related_notes_rank_by_shared_tags_with_ancestors_and_keep_newest_first() {
        const D: &str = "01DRZ3NDEKTSV4RRFFQ69G5FAV";
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        let notes = [
            note(A, "Subject", "tags: [work/rust, idea]\n"),
            note(B, "Twin", "tags: [work/rust, idea]\n"),
            note(C, "Cousin", "tags: [work/python]\n"),
            note(D, "Stranger", "tags: [home]\n"),
            note(E, "Idea", "tags: [idea]\n"),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let subject = model
            .notes
            .iter()
            .position(|n| n.title == "Subject")
            .expect("subject");
        // Twin shares three paths (work, work/rust, idea), Idea and Cousin one
        // each; Idea is newer than Cousin; Stranger shares nothing.
        assert_eq!(
            titles(&model, &model.related(subject)),
            ["Twin", "Idea", "Cousin"]
        );
        let stranger = model
            .notes
            .iter()
            .position(|n| n.title == "Stranger")
            .expect("stranger");
        assert!(model.related(stranger).is_empty());
        let untagged = [note(A, "Loose", "")];
        let loose = Model::build(&untagged, &[], &SiteOptions::default(), "Vault").expect("model");
        assert!(loose.related(0).is_empty());
    }

    #[test]
    fn a_note_is_placed_in_its_first_group_with_neighbours() {
        let notes = [
            note(A, "One", "status: open\ntags: [x]\n"),
            note(B, "Two", "status: open\n"),
            note(C, "Three", "status: open\n"),
        ];
        let views = [ViewDef::new("by-status", "status")];
        let model = model(&notes, &views, SiteOptions::default());
        // Newest first: Three, Two, One.
        let three = model.placements[0].as_ref().expect("placed");
        assert_eq!(three.section, 0);
        assert_eq!(three.path, vec![0]);
        assert_eq!(three.prev, None);
        assert_eq!(three.next, Some(1));
        let one = model.placements[2].as_ref().expect("placed");
        // `One` is also tagged, but the view section comes first.
        assert_eq!(one.section, 0);
        assert_eq!(one.prev, Some(1));
        assert_eq!(one.next, None);
        assert_eq!(model.group_at(one).value, "open");
        let trail: Vec<&str> = model.trail(one).iter().map(|g| g.value.as_str()).collect();
        assert_eq!(trail, ["open"]);
    }

    #[test]
    fn a_nested_placement_has_the_full_trail() {
        let notes = [note(A, "Deep", "tags: [a/b/c]\n")];
        let model = model(&notes, &[], SiteOptions::default());
        let placement = model.placements[0].as_ref().expect("placed");
        assert_eq!(placement.path, vec![0, 0, 0]);
        let trail: Vec<&str> = model
            .trail(placement)
            .iter()
            .map(|g| g.value.as_str())
            .collect();
        assert_eq!(trail, ["a", "a/b", "a/b/c"]);
    }

    #[test]
    fn a_note_in_no_group_has_no_placement() {
        let notes = [note(A, "Loose", "")];
        let model = model(&notes, &[], SiteOptions::default());
        assert_eq!(model.placements, vec![None]);
        assert!(model.sections[0].groups.is_empty());
    }

    #[test]
    fn the_front_page_is_the_configured_note_or_an_overview() {
        let notes = [note(A, "Home", ""), note(B, "Other", "")];
        assert_eq!(
            model(&notes, &[], SiteOptions::default()).front,
            Front::Overview
        );

        let configured = SiteOptions {
            index: Some(A.to_string()),
            ..SiteOptions::default()
        };
        let model = model(&notes, &[], configured);
        assert_eq!(model.front, Front::Note(1));
        assert!(model.warnings.is_empty());
    }

    #[test]
    fn a_configured_index_outside_the_set_warns_and_falls_back() {
        let notes = [note(B, "Other", "")];
        let configured = SiteOptions {
            index: Some(A.to_string()),
            ..SiteOptions::default()
        };
        let model = model(&notes, &[], configured);
        assert_eq!(model.front, Front::Overview);
        assert_eq!(model.warnings.len(), 1);
        assert!(model.warnings[0].contains(A), "{}", model.warnings[0]);
    }

    #[test]
    fn configured_title_and_lang_apply() {
        let options = SiteOptions {
            title: Some("Docs".to_string()),
            lang: Some("de".to_string()),
            ..SiteOptions::default()
        };
        let model = model(&[], &[], options);
        assert_eq!(model.title, "Docs");
        assert_eq!(model.lang, "de");
        assert!(model.notes.is_empty());
    }

    #[test]
    fn page_of_finds_exported_notes_only() {
        let notes = [note(A, "Home", "")];
        let model = model(&notes, &[], SiteOptions::default());
        assert_eq!(
            model.page_of(A.parse().expect("ulid")),
            Some("notes/home.html")
        );
        assert_eq!(model.page_of(B.parse().expect("ulid")), None);
    }
}
