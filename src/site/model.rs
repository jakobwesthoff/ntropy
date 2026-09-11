// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The site's structure, computed from the exported notes and the vault's
//! views before any page is rendered (ADR 0054, ADR 0056).
//!
//! The model answers every structural question the pages ask: what each
//! note's page is called, which sections the sidebar has and how their
//! groups nest, which notes a group holds and in what order, where a note
//! sits first (its breadcrumb, its previous and next neighbours), and what
//! the front page is. It is pure data over the note set, so the page paths
//! and the navigation are testable without rendering a byte of HTML.

use std::collections::BTreeMap;

use serde_yaml_ng::{Mapping, Value};

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

/// The frontmatter field holding a note's navigation settings.
pub const SITE_FIELD: &str = "site";

/// A note's `site` frontmatter table (ADR 0056): how the note sits in the
/// navigation. Every key is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteMeta {
    /// The note's position among the entries of every group holding it;
    /// for a landing note, its group's position among the parent's entries.
    pub order: Option<i64>,
    /// The name the sidebar and the pager show instead of the title; on a
    /// landing note, the group's name.
    pub label: Option<String>,
    /// Out of the sidebar, every list, and the pager; the page still
    /// exists.
    pub hidden: bool,
    /// The landing note of every group it is a member of.
    pub index: bool,
}

impl SiteMeta {
    /// Read the table from a note's frontmatter. A `site` value that is no
    /// mapping, and a key of the wrong type, are reported through `warn`
    /// and ignored; unknown keys are ignored silently.
    pub fn read(frontmatter: &Mapping, mut warn: impl FnMut(String)) -> SiteMeta {
        let mut meta = SiteMeta::default();
        let Some(table) = frontmatter.get(Value::from(SITE_FIELD)) else {
            return meta;
        };
        let Value::Mapping(table) = table else {
            warn(format!(
                "the `{SITE_FIELD}` field is not a table and is ignored"
            ));
            return meta;
        };
        for (key, value) in table {
            let Value::String(key) = key else { continue };
            match (key.as_str(), value) {
                ("order", Value::Number(n)) if n.as_i64().is_some() => {
                    meta.order = n.as_i64();
                }
                ("label", Value::String(s)) => meta.label = Some(s.clone()),
                ("hidden", Value::Bool(b)) => meta.hidden = *b,
                ("index", Value::Bool(b)) => meta.index = *b,
                ("order" | "label" | "hidden" | "index", _) => warn(format!(
                    "`{SITE_FIELD}.{key}` has the wrong type and is ignored (order takes an integer, label a string, hidden and index a boolean)"
                )),
                _ => {}
            }
        }
        meta
    }
}

/// One exported note, as the pages refer to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEntry {
    pub id: Id,
    pub title: String,
    pub created: String,
    pub tags: Vec<String>,
    /// The page path within the site: `notes/<slug>.html`, with a ULID tail
    /// on the slug where two notes share one; for a landing note, the page
    /// of the first group it lands.
    pub page: String,
    pub site: SiteMeta,
}

impl NoteEntry {
    /// What the sidebar and the pager call the note.
    pub fn name(&self) -> &str {
        self.site.label.as_deref().unwrap_or(&self.title)
    }
}

/// One entry of a group in reading order: a member note, by model index,
/// or a child group, by its position in the group's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Note(usize),
    Child(usize),
}

/// What decides an entry's place in reading order, compared field by field:
/// ordered entries before unordered, by order, notes before groups, notes
/// newest first (the model's index ascending), groups by label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ReadingKey {
    unordered: bool,
    order: i64,
    group: bool,
    newest: usize,
    label: String,
}

/// A group inside a section: one grouping value (a tag, a field value) and
/// the notes and child groups under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The full value, `programming/rust`.
    pub value: String,
    /// What the sidebar shows: the landing note's label when it has one,
    /// else the value's last segment.
    pub label: String,
    /// Page path: `<section root>/<value>/index.html`.
    pub page: String,
    /// The landing note (ADR 0056), whose title and body the group's page
    /// shows above the listing, as an index into the model's notes.
    pub landing: Option<usize>,
    /// The notes whose value is exactly this group's, in reading order, the
    /// landing note excluded, as indices into the model's notes.
    pub notes: Vec<usize>,
    /// Child groups, in reading order.
    pub children: Vec<Group>,
    /// The reading order over the notes and the children together.
    pub entries: Vec<Entry>,
    /// The group's position among its parent's entries: its landing note's
    /// order.
    pub order: Option<i64>,
}

impl Group {
    /// The notes of this group and of every group below it in reading
    /// order, each once, the group's own landing note excluded and the
    /// children's included: what the group's own page lists (ADR 0054, tag
    /// pages).
    pub fn descendants(&self) -> Vec<usize> {
        let mut all = Vec::new();
        self.collect(&mut all);
        let mut seen = std::collections::HashSet::new();
        all.retain(|index| seen.insert(*index));
        all
    }

    fn collect(&self, into: &mut Vec<usize>) {
        for entry in &self.entries {
            match *entry {
                Entry::Note(index) => into.push(index),
                Entry::Child(position) => {
                    let child = &self.children[position];
                    into.extend(child.landing);
                    child.collect(into);
                }
            }
        }
    }

    /// Put the notes and children into reading order (ADR 0056): the entries
    /// with an order first, ascending; then the notes without one, newest
    /// first; then the children without one, by label. A note and a child
    /// sharing an order keep the note first.
    fn arrange(&mut self, notes: &[NoteEntry]) {
        let mut keyed: Vec<(ReadingKey, Entry)> = Vec::new();
        for &index in &self.notes {
            let order = notes[index].site.order;
            keyed.push((
                ReadingKey {
                    unordered: order.is_none(),
                    order: order.unwrap_or(0),
                    group: false,
                    newest: index,
                    label: String::new(),
                },
                Entry::Note(index),
            ));
        }
        for (position, child) in self.children.iter().enumerate() {
            keyed.push((
                ReadingKey {
                    unordered: child.order.is_none(),
                    order: child.order.unwrap_or(0),
                    group: true,
                    newest: 0,
                    label: child.label.clone(),
                },
                Entry::Child(position),
            ));
        }
        keyed.sort_by(|a, b| a.0.cmp(&b.0));

        let mut children: Vec<Option<Group>> = std::mem::take(&mut self.children)
            .into_iter()
            .map(Some)
            .collect();
        self.notes.clear();
        self.entries.clear();
        for (_, entry) in keyed {
            match entry {
                Entry::Note(index) => {
                    self.notes.push(index);
                    self.entries.push(Entry::Note(index));
                }
                Entry::Child(position) => {
                    let child = children[position]
                        .take()
                        .expect("each child is placed exactly once");
                    self.children.push(child);
                    self.entries.push(Entry::Child(self.children.len() - 1));
                }
            }
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
    /// The top-level groups in reading order.
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

/// A group a note is the landing note of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landing {
    pub note: usize,
    pub section: usize,
    /// The path of child indices from the section's groups to the group.
    pub path: Vec<usize>,
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
    /// not in the exported set, a `site` table that does not parse.
    pub warnings: Vec<String>,
}

impl Model {
    /// Build the model. `notes` is the exported set in any order; the model
    /// orders them newest first.
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

        let mut warnings = Vec::new();
        let mut entries = Vec::with_capacity(ordered.len());
        for (note, name) in ordered.iter().zip(names) {
            let site = SiteMeta::read(&note.frontmatter, |message| {
                warnings.push(format!("{}: {message}", note.path.display()))
            });
            entries.push(NoteEntry {
                id: note.id,
                title: note.title.clone(),
                created: note.created_date()?,
                tags: note.tags.clone(),
                page: format!("notes/{name}.html"),
                site,
            });
        }

        // A hidden note belongs to no group: it is in no sidebar, no list,
        // and no pager, which is what membership would put it in.
        let values_of = |index: usize, values: Vec<String>| {
            if entries[index].site.hidden {
                Vec::new()
            } else {
                values
            }
        };

        // Sections in sidebar order: the configured views, then the tags. A
        // view over the tags field would repeat the tag section group for
        // group, so it is no section and gets no pages.
        let mut sections = Vec::new();
        for view in views.iter().filter(|view| view.field != TAGS_FIELD) {
            let root = format!("views/{}", view.name);
            let values: Vec<Vec<String>> = ordered
                .iter()
                .enumerate()
                .map(|(index, note)| values_of(index, view::group_values(note, &view.field)))
                .collect();
            sections.push(Section {
                kind: SectionKind::View {
                    field: view.field.clone(),
                },
                title: view.name.clone(),
                page: format!("{root}/index.html"),
                groups: tree(&root, &values, &entries),
                root,
            });
        }
        let tag_values: Vec<Vec<String>> = ordered
            .iter()
            .enumerate()
            .map(|(index, note)| values_of(index, note.tags.clone()))
            .collect();
        sections.push(Section {
            kind: SectionKind::Tags,
            title: "tags".to_string(),
            page: "tags/index.html".to_string(),
            groups: tree("tags", &tag_values, &entries),
            root: "tags".to_string(),
        });

        // A landing note's page is the page of the first group it lands.
        for landing in landings(&sections) {
            let entry = &mut entries[landing.note];
            if entry.page.starts_with("notes/") {
                entry.page = group_at_path(&sections, landing.section, &landing.path)
                    .page
                    .clone();
            }
        }

        let placements = placements(entries.len(), &sections);

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
    /// Notes sharing nothing, and hidden notes, are left out; ties keep the
    /// newest first. At most [`RELATED_LIMIT`] notes.
    pub fn related(&self, index: usize) -> Vec<usize> {
        let own = tag_paths(&self.notes[index].tags);
        if own.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(usize, usize)> = self
            .notes
            .iter()
            .enumerate()
            .filter(|(other, note)| *other != index && !note.site.hidden)
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
        group_at_path(&self.sections, placement.section, &placement.path)
    }

    /// The groups from the section's root down to the placement's group,
    /// outermost first: the breadcrumb.
    pub fn trail(&self, placement: &Placement) -> Vec<&Group> {
        self.trail_to(placement.section, &placement.path)
    }

    /// The groups from the section's root down the path, outermost first.
    pub fn trail_to(&self, section: usize, path: &[usize]) -> Vec<&Group> {
        let section = &self.sections[section];
        let mut trail = Vec::new();
        let mut group = &section.groups[path[0]];
        trail.push(group);
        for &index in &path[1..] {
            group = &group.children[index];
            trail.push(group);
        }
        trail
    }

    /// Every group with a landing note, in sidebar order.
    pub fn landings(&self) -> Vec<Landing> {
        landings(&self.sections)
    }

    /// The notes the generated front page lists: the newest visible ones.
    pub fn recent(&self) -> Vec<usize> {
        self.notes
            .iter()
            .enumerate()
            .filter(|(_, note)| !note.site.hidden)
            .map(|(index, _)| index)
            .take(FRONT_PAGE_RECENT)
            .collect()
    }
}

fn group_at_path<'a>(sections: &'a [Section], section: usize, path: &[usize]) -> &'a Group {
    let mut group = &sections[section].groups[path[0]];
    for &index in &path[1..] {
        group = &group.children[index];
    }
    group
}

/// Every group with a landing note, sections in order and groups depth-first
/// in reading order.
fn landings(sections: &[Section]) -> Vec<Landing> {
    fn visit(groups: &[Group], section: usize, path: &mut Vec<usize>, out: &mut Vec<Landing>) {
        for (position, group) in groups.iter().enumerate() {
            path.push(position);
            if let Some(note) = group.landing {
                out.push(Landing {
                    note,
                    section,
                    path: path.clone(),
                });
            }
            visit(&group.children, section, path, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        visit(&section.groups, index, &mut Vec::new(), &mut out);
    }
    out
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
/// value names. The first member marked `index` is the group's landing
/// note and lends it its label and order; the entries of every group, and
/// the top-level groups, are put into reading order.
fn tree(root: &str, values_per_note: &[Vec<String>], notes: &[NoteEntry]) -> Vec<Group> {
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

    fn convert(
        root: &str,
        prefix: &str,
        nodes: BTreeMap<String, Node>,
        notes: &[NoteEntry],
    ) -> Vec<Group> {
        let mut groups: Vec<Group> = nodes
            .into_iter()
            .map(|(segment, node)| {
                let value = if prefix.is_empty() {
                    segment.clone()
                } else {
                    format!("{prefix}/{segment}")
                };
                let mut members = node.notes;
                let landing = members
                    .iter()
                    .copied()
                    .find(|&index| notes[index].site.index);
                members.retain(|&index| Some(index) != landing);
                let (label, order) = match landing {
                    Some(index) => (
                        notes[index].site.label.clone().unwrap_or(segment),
                        notes[index].site.order,
                    ),
                    None => (segment, None),
                };
                let mut group = Group {
                    page: format!("{root}/{value}/index.html"),
                    children: convert(root, &value, node.children, notes),
                    notes: members,
                    entries: Vec::new(),
                    landing,
                    label,
                    order,
                    value,
                };
                group.arrange(notes);
                group
            })
            .collect();
        // The top-level groups follow the same rule as entries, among
        // themselves: ordered ones first, the rest by label.
        groups.sort_by(|a, b| {
            (a.order.is_none(), a.order.unwrap_or(0), &a.label).cmp(&(
                b.order.is_none(),
                b.order.unwrap_or(0),
                &b.label,
            ))
        });
        groups
    }

    convert(root, "", top, notes)
}

/// Each note's first placement in sidebar order: sections in order, groups
/// depth-first in reading order, a group's landing note before its entries.
/// Previous and next are the neighbours in that reading order within the
/// section, across group boundaries.
fn placements(count: usize, sections: &[Section]) -> Vec<Option<Placement>> {
    let mut placements: Vec<Option<Placement>> = vec![None; count];

    fn visit(group: &Group, path: &mut Vec<usize>, out: &mut Vec<(usize, Vec<usize>)>) {
        if let Some(note) = group.landing {
            out.push((note, path.clone()));
        }
        for entry in &group.entries {
            match *entry {
                Entry::Note(note) => out.push((note, path.clone())),
                Entry::Child(child) => {
                    path.push(child);
                    visit(&group.children[child], path, out);
                    path.pop();
                }
            }
        }
    }

    for (index, section) in sections.iter().enumerate() {
        let mut order = Vec::new();
        for (position, group) in section.groups.iter().enumerate() {
            visit(group, &mut vec![position], &mut order);
        }
        for (at, (note, path)) in order.iter().enumerate() {
            if placements[*note].is_none() {
                placements[*note] = Some(Placement {
                    section: index,
                    path: path.clone(),
                    prev: at.checked_sub(1).map(|p| order[p].0),
                    next: order.get(at + 1).map(|n| n.0),
                });
            }
        }
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
        // The group page lists descendants too, in reading order: the
        // group's own notes, then each child's.
        assert_eq!(
            titles(&model, &programming.descendants()),
            ["Prog", "Both", "Rust"]
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
    fn the_site_table_reads_every_key_and_warns_on_wrong_types() {
        let warnings = std::cell::RefCell::new(Vec::new());
        let warn = |message: String| warnings.borrow_mut().push(message);
        let full = note(
            A,
            "Full",
            "site:\n  order: 2\n  label: Start here\n  hidden: true\n  index: true\n  extra: ignored\n",
        );
        assert_eq!(
            SiteMeta::read(&full.frontmatter, warn),
            SiteMeta {
                order: Some(2),
                label: Some("Start here".to_string()),
                hidden: true,
                index: true,
            }
        );
        let none = note(B, "None", "status: open\n");
        assert_eq!(SiteMeta::read(&none.frontmatter, warn), SiteMeta::default());
        assert!(warnings.borrow().is_empty(), "{warnings:?}");

        let wrong = note(C, "Wrong", "site:\n  order: two\n  hidden: yes please\n");
        assert_eq!(
            SiteMeta::read(&wrong.frontmatter, warn),
            SiteMeta::default()
        );
        let scalar = note(C, "Scalar", "site: 3\n");
        assert_eq!(
            SiteMeta::read(&scalar.frontmatter, warn),
            SiteMeta::default()
        );
        let warnings = warnings.into_inner();
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(
            warnings[0].contains("`site.order` has the wrong type"),
            "{}",
            warnings[0]
        );
        assert!(
            warnings[1].contains("`site.hidden` has the wrong type"),
            "{}",
            warnings[1]
        );
        assert!(warnings[2].contains("not a table"), "{}", warnings[2]);
    }

    #[test]
    fn a_bad_site_table_is_a_model_warning_naming_the_note() {
        let notes = [note(A, "Wrong", "site:\n  order: two\n")];
        let model = model(&notes, &[], SiteOptions::default());
        assert_eq!(model.warnings.len(), 1);
        assert!(
            model.warnings[0].contains("wrong.md"),
            "{}",
            model.warnings[0]
        );
        assert!(
            model.warnings[0].contains("`site.order`"),
            "{}",
            model.warnings[0]
        );
    }

    /// Five notes under one tag: two ordered, two not, one the landing note
    /// of a child group with an order of its own, plus a child group with
    /// no landing note.
    fn ordered_notes() -> [Note; 5] {
        const D: &str = "01DRZ3NDEKTSV4RRFFQ69G5FAV";
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        [
            note(A, "One", "tags: [t]\nsite:\n  order: 2\n"),
            note(B, "Two", "tags: [t]\n"),
            note(C, "Three", "tags: [t]\nsite:\n  order: 1\n"),
            note(D, "Four", "tags: [t/zeta]\n"),
            note(
                E,
                "Five",
                "tags: [t/alpha]\nsite:\n  index: true\n  label: Alpha!\n  order: 0\n",
            ),
        ]
    }

    #[test]
    fn entries_read_ordered_first_then_newest_notes_then_groups_by_label() {
        let notes = ordered_notes();
        let model = model(&notes, &[], SiteOptions::default());
        let t = &model.sections[0].groups[0];
        assert_eq!(t.value, "t");
        // The child `alpha` carries its landing note's order 0, so it comes
        // before the ordered notes; the unordered note follows them, newest
        // first; the unordered child closes.
        let entries: Vec<String> = t
            .entries
            .iter()
            .map(|entry| match *entry {
                Entry::Note(index) => model.notes[index].title.clone(),
                Entry::Child(child) => format!("[{}]", t.children[child].label),
            })
            .collect();
        assert_eq!(entries, ["[Alpha!]", "Three", "One", "Two", "[zeta]"]);
        assert_eq!(titles(&model, &t.notes), ["Three", "One", "Two"]);
        let labels: Vec<&str> = t.children.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Alpha!", "zeta"]);

        let alpha = &t.children[0];
        assert_eq!(alpha.value, "t/alpha");
        assert_eq!(alpha.order, Some(0));
        assert_eq!(
            alpha.landing.map(|index| model.notes[index].title.as_str()),
            Some("Five")
        );
        // The landing note is the page, not an entry, and its own page is
        // the group's.
        assert!(alpha.notes.is_empty());
        assert!(alpha.entries.is_empty());
        assert_eq!(
            model.notes[alpha.landing.expect("landing")].page,
            "tags/t/alpha/index.html"
        );
        // The listing of `t` reads the children's landing notes and notes in
        // reading order.
        assert_eq!(
            titles(&model, &t.descendants()),
            ["Five", "Three", "One", "Two", "Four"]
        );
        assert!(alpha.descendants().is_empty());
    }

    #[test]
    fn neighbours_follow_the_reading_order_across_groups() {
        let notes = ordered_notes();
        let model = model(&notes, &[], SiteOptions::default());
        let by_title = |title: &str| {
            model
                .notes
                .iter()
                .position(|n| n.title == title)
                .expect("the note exists")
        };
        let placed = |title: &str| model.placements[by_title(title)].as_ref().expect("placed");
        // Reading order: Five (alpha's landing), Three, One, Two, Four.
        assert_eq!(placed("Five").prev, None);
        assert_eq!(placed("Five").next, Some(by_title("Three")));
        assert_eq!(placed("Five").path, vec![0, 0]);
        assert_eq!(placed("Three").prev, Some(by_title("Five")));
        assert_eq!(placed("Two").next, Some(by_title("Four")));
        assert_eq!(placed("Four").prev, Some(by_title("Two")));
        assert_eq!(placed("Four").next, None);
        assert_eq!(placed("Four").path, vec![0, 1]);
        let trail: Vec<&str> = model
            .trail(placed("Four"))
            .iter()
            .map(|g| g.label.as_str())
            .collect();
        assert_eq!(trail, ["t", "zeta"]);
    }

    #[test]
    fn top_level_groups_read_ordered_landings_first_then_labels() {
        let notes = [
            note(A, "A note", "tags: [a]\n"),
            note(
                B,
                "B landing",
                "tags: [b]\nsite:\n  index: true\n  order: 1\n",
            ),
            note(C, "C note", "tags: [c]\n"),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let labels: Vec<&str> = model.sections[0]
            .groups
            .iter()
            .map(|g| g.label.as_str())
            .collect();
        assert_eq!(labels, ["b", "a", "c"]);
        let landings = model.landings();
        assert_eq!(landings.len(), 1);
        assert_eq!(landings[0].section, 0);
        assert_eq!(landings[0].path, vec![0]);
        assert_eq!(model.notes[landings[0].note].title, "B landing");
    }

    #[test]
    fn a_landing_note_lands_every_group_it_is_in_and_takes_the_first_page() {
        let notes = [
            note(A, "Home", "tags: [b, a]\nsite:\n  index: true\n"),
            note(B, "Other", "tags: [a]\n"),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let landings = model.landings();
        assert_eq!(landings.len(), 2);
        let home = model
            .notes
            .iter()
            .position(|n| n.title == "Home")
            .expect("home");
        assert_eq!(model.notes[home].page, "tags/a/index.html");
        assert_eq!(
            model.page_of(A.parse().expect("ulid")),
            Some("tags/a/index.html")
        );
        assert_eq!(model.sections[0].groups[1].landing, Some(home));
        // Without a label the group keeps its segment.
        assert_eq!(model.sections[0].groups[0].label, "a");
    }

    #[test]
    fn hidden_notes_join_no_group_and_no_list() {
        let notes = [
            note(A, "Shown", "tags: [t]\n"),
            note(
                B,
                "Hidden",
                "tags: [t, only]\nstatus: open\nsite:\n  hidden: true\n",
            ),
            note(C, "Other", "tags: [t]\n"),
        ];
        let views = [ViewDef::new("by-status", "status")];
        let model = model(&notes, &views, SiteOptions::default());
        let hidden = model
            .notes
            .iter()
            .position(|n| n.title == "Hidden")
            .expect("hidden");
        // The page exists, the note sits nowhere.
        assert_eq!(model.notes[hidden].page, "notes/hidden.html");
        assert_eq!(model.placements[hidden], None);
        assert!(
            model.sections[0].groups.is_empty(),
            "the view has no groups"
        );
        let tags = &model.sections[1];
        let labels: Vec<&str> = tags.groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, ["t"], "a tag only hidden notes carry has no group");
        assert_eq!(titles(&model, &tags.groups[0].notes), ["Other", "Shown"]);
        // Neither related nor recent list it.
        let shown = model
            .notes
            .iter()
            .position(|n| n.title == "Shown")
            .expect("shown");
        assert_eq!(titles(&model, &model.related(shown)), ["Other"]);
        assert_eq!(titles(&model, &model.recent()), ["Other", "Shown"]);
        // The neighbours skip it.
        let other = model.placements[model
            .notes
            .iter()
            .position(|n| n.title == "Other")
            .expect("other")]
        .as_ref()
        .expect("placed");
        assert_eq!(other.next, Some(shown));
    }

    #[test]
    fn a_label_names_the_note_and_the_title_stays() {
        let notes = [
            note(A, "Long Title", "site:\n  label: Short\n"),
            note(B, "Plain", ""),
        ];
        let model = model(&notes, &[], SiteOptions::default());
        let long = model
            .notes
            .iter()
            .find(|n| n.title == "Long Title")
            .expect("note");
        assert_eq!(long.name(), "Short");
        let plain = model
            .notes
            .iter()
            .find(|n| n.title == "Plain")
            .expect("note");
        assert_eq!(plain.name(), "Plain");
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
