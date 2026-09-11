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
use crate::query::Query;
use crate::text::{slug, tag};
use crate::view::{self, ViewDef};

use super::SiteOptions;
use super::options::{NavEntry, NavEntryKind, NavSection};

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
    /// Whether the note's page ends with its related notes, overriding the
    /// site's setting.
    pub related: Option<bool>,
    /// On a landing note, whether the group's page lists the group's
    /// contents below the note.
    pub listing: bool,
    /// The theme template that renders the note's page instead of
    /// `page.html`, by its name without the `.html` (ADR 0058).
    pub template: Option<String>,
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
                ("related", Value::Bool(b)) => meta.related = Some(*b),
                ("listing", Value::Bool(b)) => meta.listing = *b,
                ("template", Value::String(s)) => meta.template = Some(s.clone()),
                (
                    "order" | "label" | "hidden" | "index" | "related" | "listing" | "template",
                    _,
                ) => warn(format!(
                    "`{SITE_FIELD}.{key}` has the wrong type and is ignored (order takes an integer, label and template a string, the others a boolean)"
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

/// One item of the sidebar (ADR 0056).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavItem {
    /// A note, by model index, shown by `label` or by its own name.
    Note { index: usize, label: Option<String> },
    /// A group of a section with its whole subtree, shown by `label` or by
    /// its own.
    Group {
        section: usize,
        path: Vec<usize>,
        label: Option<String>,
    },
    /// A group assembled by hand, with no page of its own.
    Curated { label: String, items: Vec<NavItem> },
}

/// One section of the sidebar: a section of the site, the root group, or a
/// `[[site.nav]]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSection {
    pub label: String,
    /// The section's own page, which its title links to, when it has one.
    pub page: Option<String>,
    /// The group the section is, by section and path, when it is one: a
    /// child of the sidebar's root. Its breadcrumb is then empty, nothing
    /// in the sidebar being above it.
    pub group: Option<(usize, Vec<usize>)>,
    /// Drawn as a cloud of the items' groups with counts, the tag section's
    /// shape, rather than as a tree.
    pub cloud: bool,
    pub items: Vec<NavItem>,
}

/// One step of a breadcrumb: a page of the site, or a hand-assembled nav
/// group that has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub label: String,
    pub page: Option<String>,
}

/// Where a note sits first in sidebar order, which fixes its breadcrumb and
/// its previous and next neighbours (ADR 0054).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The breadcrumb: the sidebar section, then each step down to the
    /// group holding the note.
    pub trail: Vec<Crumb>,
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
    /// The sidebar: the sections, the root group's entries, or the nav
    /// table (ADR 0056).
    pub sidebar: Vec<SidebarSection>,
    /// Per note, its first placement, or `None` for a note in no group.
    pub placements: Vec<Option<Placement>>,
    /// Per group the sidebar reaches, keyed by section and path, the
    /// breadcrumb above it.
    group_trails: BTreeMap<(usize, Vec<usize>), Vec<Crumb>>,
    pub front: Front,
    /// Whether note pages end with their related notes, unless a note says
    /// otherwise.
    pub related: bool,
    /// Non-fatal findings while building: a configured index note that is
    /// not in the exported set, a `site` table that does not parse.
    pub warnings: Vec<String>,
}

impl Model {
    /// Build the model for an export without a query. `notes` is the
    /// exported set in any order; the model orders them newest first.
    pub fn build(
        notes: &[Note],
        views: &[ViewDef],
        options: &SiteOptions,
        fallback_title: &str,
    ) -> Result<Model, crate::error::Error> {
        Model::build_for(notes, views, options, None, fallback_title)
    }

    /// Build the model. `query` is the export's query, whose single `tag:`
    /// predicate roots the sidebar when the options name no root.
    pub fn build_for(
        notes: &[Note],
        views: &[ViewDef],
        options: &SiteOptions,
        query: Option<&Query>,
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

        let sidebar = sidebar(options, query, &entries, &sections, &mut warnings);
        let (placements, group_trails) = placements(entries.len(), &sections, &sidebar);

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
            sidebar,
            placements,
            group_trails,
            front,
            related: options.related(),
            warnings,
        })
    }

    /// Whether the page of the note at `index` ends with its related notes:
    /// the note's own `site.related`, else the site's setting.
    pub fn shows_related(&self, index: usize) -> bool {
        self.notes[index].site.related.unwrap_or(self.related)
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

    /// The group at `path` in the section.
    pub fn group_at(&self, section: usize, path: &[usize]) -> &Group {
        group_at_path(&self.sections, section, path)
    }

    /// The breadcrumb above the group at `path`: where the sidebar first
    /// reaches it, or, for a group the sidebar never reaches, its section
    /// and the groups above it.
    pub fn group_trail(&self, section: usize, path: &[usize]) -> Vec<Crumb> {
        if let Some(trail) = self.group_trails.get(&(section, path.to_vec())) {
            return trail.clone();
        }
        let owner = &self.sections[section];
        let mut trail = vec![Crumb {
            label: owner.title.clone(),
            page: Some(owner.page.clone()),
        }];
        for depth in 1..path.len() {
            let group = group_at_path(&self.sections, section, &path[..depth]);
            trail.push(Crumb {
                label: group.label.clone(),
                page: Some(group.page.clone()),
            });
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

/// The sidebar (ADR 0056): the nav table when the options hold one, else
/// the root group's child groups as sections when a root is configured or
/// the query is a single `tag:` predicate, else one section per section of
/// the site. A root that names no group of the site is a warning, and the
/// sidebar is the default.
fn sidebar(
    options: &SiteOptions,
    query: Option<&Query>,
    notes: &[NoteEntry],
    sections: &[Section],
    warnings: &mut Vec<String>,
) -> Vec<SidebarSection> {
    if !options.nav.is_empty() {
        return curated_sidebar(&options.nav, notes, sections, warnings);
    }
    let root = options.root.clone().or_else(|| match query {
        Some(Query::Tag(value)) => Some(format!("tags/{}", tag::normalize(value))),
        _ => None,
    });
    if let Some(root) = root {
        match find_page(sections, &root) {
            Some((section, path)) => return root_sections(sections, section, path),
            None => warnings.push(format!(
                "the sidebar root `{root}` is not a tag or view group page of the site; the sidebar shows the views and the tags"
            )),
        }
    }
    default_sidebar(sections)
}

/// One sidebar section per section of the site, the tag section as a
/// cloud.
fn default_sidebar(sections: &[Section]) -> Vec<SidebarSection> {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| SidebarSection {
            label: section.title.clone(),
            page: Some(section.page.clone()),
            group: None,
            cloud: section.kind == SectionKind::Tags,
            items: group_items(section, index, &[]),
        })
        .collect()
}

/// The rooted sidebar: one section per child group of the root, titled by
/// the group and holding its entries, so the root itself never wraps the
/// navigation. The root's own notes come first under the root's label, and
/// a root without child groups is that one section.
fn root_sections(sections: &[Section], section: usize, path: Vec<usize>) -> Vec<SidebarSection> {
    let root = group_at_path(sections, section, &path);
    let mut sidebar = Vec::new();
    if !root.notes.is_empty() || root.children.is_empty() {
        sidebar.push(SidebarSection {
            label: root.label.clone(),
            page: Some(root.page.clone()),
            group: Some((section, path.clone())),
            cloud: false,
            items: root
                .entries
                .iter()
                .filter_map(|entry| match *entry {
                    Entry::Note(index) => Some(NavItem::Note { index, label: None }),
                    Entry::Child(_) => None,
                })
                .collect(),
        });
    }
    for entry in &root.entries {
        let Entry::Child(child) = *entry else {
            continue;
        };
        let child_path = [path.as_slice(), &[child]].concat();
        let group = &root.children[child];
        sidebar.push(SidebarSection {
            label: group.label.clone(),
            page: Some(group.page.clone()),
            group: Some((section, child_path.clone())),
            cloud: false,
            items: entry_items(group, section, &child_path),
        });
    }
    sidebar
}

/// A group's entries as nav items: its notes, and its children as groups.
fn entry_items(group: &Group, section: usize, path: &[usize]) -> Vec<NavItem> {
    group
        .entries
        .iter()
        .map(|entry| match *entry {
            Entry::Note(index) => NavItem::Note { index, label: None },
            Entry::Child(child) => NavItem::Group {
                section,
                path: [path, &[child]].concat(),
                label: None,
            },
        })
        .collect()
}

/// A section's top-level groups as nav items.
fn group_items(section: &Section, index: usize, _path: &[usize]) -> Vec<NavItem> {
    (0..section.groups.len())
        .map(|position| NavItem::Group {
            section: index,
            path: vec![position],
            label: None,
        })
        .collect()
}

/// The section and path of the group page `tags/<path>` or
/// `views/<name>/<group>` names, if the site has it.
fn find_page(sections: &[Section], page: &str) -> Option<(usize, Vec<usize>)> {
    let mut segments = page.trim_matches('/').split('/');
    let (section, value) = match segments.next()? {
        "tags" => {
            let section = sections
                .iter()
                .position(|section| section.kind == SectionKind::Tags)?;
            (section, segments.collect::<Vec<_>>())
        }
        "views" => {
            let name = segments.next()?;
            let section = sections.iter().position(|section| {
                matches!(section.kind, SectionKind::View { .. }) && section.title == name
            })?;
            (section, segments.collect::<Vec<_>>())
        }
        _ => return None,
    };
    if value.is_empty() {
        return None;
    }
    let path = find_group(&sections[section].groups, &value.join("/"))?;
    Some((section, path))
}

/// The path of child indices to the group with `value`, which lies at the
/// depth of the value's segments.
fn find_group(groups: &[Group], value: &str) -> Option<Vec<usize>> {
    for (position, group) in groups.iter().enumerate() {
        if group.value == value {
            return Some(vec![position]);
        }
        if value.starts_with(&format!("{}/", group.value)) {
            let mut path = vec![position];
            path.extend(find_group(&group.children, value)?);
            return Some(path);
        }
    }
    None
}

/// The nav table as the sidebar. An item naming a note that is not
/// exported or is hidden, a tag or view group the site has no page for, a
/// view that is not configured, a group without a label, or keys that
/// name nothing or several things, is a warning and is left out.
fn curated_sidebar(
    nav: &[NavSection],
    notes: &[NoteEntry],
    sections: &[Section],
    warnings: &mut Vec<String>,
) -> Vec<SidebarSection> {
    nav.iter()
        .map(|section| SidebarSection {
            label: section.label.clone(),
            page: None,
            group: None,
            cloud: false,
            items: nav_items(&section.items, notes, sections, warnings),
        })
        .collect()
}

fn nav_items(
    entries: &[NavEntry],
    notes: &[NoteEntry],
    sections: &[Section],
    warnings: &mut Vec<String>,
) -> Vec<NavItem> {
    let mut items = Vec::new();
    for entry in entries {
        let kind = match entry.kind() {
            Ok(kind) => kind,
            Err(reason) => {
                warnings.push(format!("[site.nav]: {reason}; the item is left out"));
                continue;
            }
        };
        let label = entry.label.clone();
        let tags_section = sections
            .iter()
            .position(|section| section.kind == SectionKind::Tags)
            .expect("the tag section always exists");
        let item = match kind {
            NavEntryKind::Note(id) => {
                match notes
                    .iter()
                    .position(|note| note.id.to_string().eq_ignore_ascii_case(id))
                {
                    Some(index) if notes[index].site.hidden => {
                        warnings.push(format!(
                            "[site.nav]: note {id} is hidden; the item is left out"
                        ));
                        continue;
                    }
                    Some(index) => NavItem::Note { index, label },
                    None => {
                        warnings.push(format!(
                            "[site.nav]: note {id} is not among the exported notes; the item is left out"
                        ));
                        continue;
                    }
                }
            }
            NavEntryKind::Tag(value) => {
                match find_group(&sections[tags_section].groups, &tag::normalize(value)) {
                    Some(path) => NavItem::Group {
                        section: tags_section,
                        path,
                        label,
                    },
                    None => {
                        warnings.push(format!(
                            "[site.nav]: no exported note carries the tag `{value}`; the item is left out"
                        ));
                        continue;
                    }
                }
            }
            NavEntryKind::View { name, group } => {
                let Some(section) = sections.iter().position(|section| {
                    matches!(section.kind, SectionKind::View { .. }) && section.title == name
                }) else {
                    warnings.push(format!(
                        "[site.nav]: `{name}` is not a configured view; the item is left out"
                    ));
                    continue;
                };
                match group {
                    Some(value) => {
                        match find_group(&sections[section].groups, &tag::normalize(value)) {
                            Some(path) => NavItem::Group {
                                section,
                                path,
                                label,
                            },
                            None => {
                                warnings.push(format!(
                                    "[site.nav]: the view `{name}` has no group `{value}`; the item is left out"
                                ));
                                continue;
                            }
                        }
                    }
                    None => NavItem::Curated {
                        label: label.unwrap_or_else(|| name.to_string()),
                        items: group_items(&sections[section], section, &[]),
                    },
                }
            }
            NavEntryKind::Tags => NavItem::Curated {
                label: label.unwrap_or_else(|| sections[tags_section].title.clone()),
                items: group_items(&sections[tags_section], tags_section, &[]),
            },
            NavEntryKind::Group(inner) => {
                let Some(label) = label else {
                    warnings.push(
                        "[site.nav]: a group of items needs a `label`; the item is left out"
                            .to_string(),
                    );
                    continue;
                };
                NavItem::Curated {
                    label,
                    items: nav_items(inner, notes, sections, warnings),
                }
            }
        };
        items.push(item);
    }
    items
}

/// Each note's first placement in sidebar order, and the breadcrumb above
/// each group the sidebar reaches: sections in order, items in order, a
/// group's landing note before its entries, its entries in reading order.
/// Previous and next are the neighbours in that reading order within the
/// sidebar section, across group boundaries.
#[allow(clippy::type_complexity)]
fn placements(
    count: usize,
    sections: &[Section],
    sidebar: &[SidebarSection],
) -> (
    Vec<Option<Placement>>,
    BTreeMap<(usize, Vec<usize>), Vec<Crumb>>,
) {
    let mut placements: Vec<Option<Placement>> = vec![None; count];
    let mut group_trails = BTreeMap::new();

    fn visit(
        items: &[NavItem],
        sections: &[Section],
        trail: &mut Vec<Crumb>,
        out: &mut Vec<(usize, Vec<Crumb>)>,
        group_trails: &mut BTreeMap<(usize, Vec<usize>), Vec<Crumb>>,
    ) {
        for item in items {
            match item {
                NavItem::Note { index, .. } => out.push((*index, trail.clone())),
                NavItem::Group {
                    section,
                    path,
                    label,
                } => {
                    let group = group_at_path(sections, *section, path);
                    group_trails
                        .entry((*section, path.clone()))
                        .or_insert_with(|| trail.clone());
                    if let Some(landing) = group.landing {
                        out.push((landing, trail.clone()));
                    }
                    trail.push(Crumb {
                        label: label.clone().unwrap_or_else(|| group.label.clone()),
                        page: Some(group.page.clone()),
                    });
                    visit(
                        &entry_items(group, *section, path),
                        sections,
                        trail,
                        out,
                        group_trails,
                    );
                    trail.pop();
                }
                NavItem::Curated { label, items } => {
                    trail.push(Crumb {
                        label: label.clone(),
                        page: None,
                    });
                    visit(items, sections, trail, out, group_trails);
                    trail.pop();
                }
            }
        }
    }

    for section in sidebar {
        if let Some(group) = &section.group {
            group_trails.entry(group.clone()).or_default();
        }
        let mut order = Vec::new();
        let mut trail = vec![Crumb {
            label: section.label.clone(),
            page: section.page.clone(),
        }];
        visit(
            &section.items,
            sections,
            &mut trail,
            &mut order,
            &mut group_trails,
        );
        for (at, (note, trail)) in order.iter().enumerate() {
            if placements[*note].is_none() {
                placements[*note] = Some(Placement {
                    trail: trail.clone(),
                    prev: at.checked_sub(1).map(|p| order[p].0),
                    next: order.get(at + 1).map(|n| n.0),
                });
            }
        }
    }
    (placements, group_trails)
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
        assert_eq!(labels(&three.trail), ["by-status", "open"]);
        assert_eq!(three.prev, None);
        assert_eq!(three.next, Some(1));
        let one = model.placements[2].as_ref().expect("placed");
        // `One` is also tagged, but the view section comes first.
        assert_eq!(
            one.trail,
            [
                Crumb {
                    label: "by-status".to_string(),
                    page: Some("views/by-status/index.html".to_string()),
                },
                Crumb {
                    label: "open".to_string(),
                    page: Some("views/by-status/open/index.html".to_string()),
                },
            ]
        );
        assert_eq!(one.prev, Some(1));
        assert_eq!(one.next, None);
        assert_eq!(model.group_at(0, &[0]).value, "open");
    }

    fn labels(trail: &[Crumb]) -> Vec<&str> {
        trail.iter().map(|crumb| crumb.label.as_str()).collect()
    }

    #[test]
    fn a_nested_placement_has_the_full_trail() {
        let notes = [note(A, "Deep", "tags: [a/b/c]\n")];
        let model = model(&notes, &[], SiteOptions::default());
        let placement = model.placements[0].as_ref().expect("placed");
        assert_eq!(labels(&placement.trail), ["tags", "a", "b", "c"]);
        assert_eq!(
            placement.trail[3].page.as_deref(),
            Some("tags/a/b/c/index.html")
        );
        // The group pages above carry the same trail, cut at their level.
        assert_eq!(labels(&model.group_trail(0, &[0, 0])), ["tags", "a"]);
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
            "site:\n  order: 2\n  label: Start here\n  hidden: true\n  index: true\n  related: false\n  listing: true\n  template: splash\n  extra: ignored\n",
        );
        assert_eq!(
            SiteMeta::read(&full.frontmatter, warn),
            SiteMeta {
                order: Some(2),
                label: Some("Start here".to_string()),
                hidden: true,
                index: true,
                related: Some(false),
                listing: true,
                template: Some("splash".to_string()),
            }
        );
        let none = note(B, "None", "status: open\n");
        assert_eq!(SiteMeta::read(&none.frontmatter, warn), SiteMeta::default());
        assert!(warnings.borrow().is_empty(), "{warnings:?}");

        let wrong = note(
            C,
            "Wrong",
            "site:\n  order: two\n  hidden: yes please\n  related: sometimes\n  template: 3\n",
        );
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
        assert_eq!(warnings.len(), 5, "{warnings:?}");
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
        assert!(
            warnings[2].contains("`site.related` has the wrong type"),
            "{}",
            warnings[2]
        );
        assert!(
            warnings[3].contains("`site.template` has the wrong type"),
            "{}",
            warnings[3]
        );
        assert!(warnings[4].contains("not a table"), "{}", warnings[4]);
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
        // A landing note's breadcrumb stops above its group, whose page it
        // is.
        assert_eq!(labels(&placed("Five").trail), ["tags", "t"]);
        assert_eq!(placed("Three").prev, Some(by_title("Five")));
        assert_eq!(placed("Two").next, Some(by_title("Four")));
        assert_eq!(placed("Four").prev, Some(by_title("Two")));
        assert_eq!(placed("Four").next, None);
        assert_eq!(labels(&placed("Four").trail), ["tags", "t", "zeta"]);
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

    /// A docs tree under `docs/…` beside an unrelated tag and a view, the
    /// shape the sidebar settings are for.
    fn docs_notes() -> [Note; 5] {
        const D: &str = "01DRZ3NDEKTSV4RRFFQ69G5FAV";
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        [
            note(
                A,
                "Basics",
                "tags: [docs/start]\nsite:\n  index: true\n  label: Getting Started\n  order: 1\n",
            ),
            note(
                B,
                "Install",
                "tags: [docs/start]\nstatus: open\nsite:\n  order: 2\n",
            ),
            note(C, "Manifest", "tags: [docs/dev]\nsite:\n  order: 1\n"),
            note(D, "Aside", "tags: [misc]\nstatus: open\n"),
            note(E, "Hidden", "tags: [docs/start]\nsite:\n  hidden: true\n"),
        ]
    }

    fn sidebar_labels(model: &Model) -> Vec<String> {
        fn walk(model: &Model, items: &[NavItem], depth: usize, out: &mut Vec<String>) {
            for item in items {
                match item {
                    NavItem::Note { index, label } => out.push(format!(
                        "{}{}",
                        "  ".repeat(depth),
                        label.as_deref().unwrap_or(model.notes[*index].name())
                    )),
                    NavItem::Group {
                        section,
                        path,
                        label,
                    } => {
                        let group = model.group_at(*section, path);
                        out.push(format!(
                            "{}[{}]",
                            "  ".repeat(depth),
                            label.as_deref().unwrap_or(&group.label)
                        ));
                        let inner: Vec<NavItem> = group
                            .entries
                            .iter()
                            .map(|entry| match *entry {
                                Entry::Note(index) => NavItem::Note { index, label: None },
                                Entry::Child(child) => NavItem::Group {
                                    section: *section,
                                    path: [path.as_slice(), &[child]].concat(),
                                    label: None,
                                },
                            })
                            .collect();
                        walk(model, &inner, depth + 1, out);
                    }
                    NavItem::Curated { label, items } => {
                        out.push(format!("{}<{label}>", "  ".repeat(depth)));
                        walk(model, items, depth + 1, out);
                    }
                }
            }
        }
        let mut out = Vec::new();
        for section in &model.sidebar {
            out.push(format!("# {}", section.label));
            walk(model, &section.items, 0, &mut out);
        }
        out
    }

    #[test]
    fn the_default_sidebar_is_one_section_per_section() {
        let notes = docs_notes();
        let views = [ViewDef::new("by-status", "status")];
        let model = model(&notes, &views, SiteOptions::default());
        assert_eq!(
            sidebar_labels(&model),
            [
                "# by-status",
                "[open]",
                "  Install",
                "  Aside",
                "# tags",
                "[docs]",
                "  [Getting Started]",
                "    Install",
                "  [dev]",
                "    Manifest",
                "[misc]",
                "  Aside",
            ]
        );
        assert_eq!(
            model.sidebar[0].page,
            Some("views/by-status/index.html".to_string())
        );
        assert!(!model.sidebar[0].cloud);
        assert!(model.sidebar[1].cloud);
        assert!(model.warnings.is_empty(), "{:?}", model.warnings);
    }

    #[test]
    fn a_configured_root_shows_that_groups_entries_and_starts_the_breadcrumb_there() {
        let notes = docs_notes();
        let views = [ViewDef::new("by-status", "status")];
        let options = SiteOptions {
            root: Some("tags/docs".to_string()),
            ..SiteOptions::default()
        };
        let model = model(&notes, &views, options);
        // The root's child groups are the sections; the root itself, which
        // holds no note of its own, wraps nothing.
        assert_eq!(
            sidebar_labels(&model),
            ["# Getting Started", "Install", "# dev", "Manifest"]
        );
        assert_eq!(
            model.sidebar[0].page,
            Some("tags/docs/start/index.html".to_string())
        );
        assert_eq!(
            model.sidebar[1].page,
            Some("tags/docs/dev/index.html".to_string())
        );
        assert!(model.warnings.is_empty(), "{:?}", model.warnings);
        // Placements come from the rooted sidebar, so Install's breadcrumb
        // starts at `docs` and ignores the view it is also in; Aside sits
        // nowhere.
        let install = model.placements[model
            .notes
            .iter()
            .position(|n| n.title == "Install")
            .expect("install")]
        .as_ref()
        .expect("placed");
        assert_eq!(labels(&install.trail), ["Getting Started"]);
        assert_eq!(
            install.trail[0].page.as_deref(),
            Some("tags/docs/start/index.html")
        );
        let aside = model
            .notes
            .iter()
            .position(|n| n.title == "Aside")
            .expect("aside");
        assert_eq!(model.placements[aside], None);
        // A section's own group page has nothing above it; one outside the
        // root keeps its section's chain.
        assert!(labels(&model.group_trail(1, &[0, 0])).is_empty());
        assert_eq!(labels(&model.group_trail(1, &[1])), ["tags"]);
        assert_eq!(labels(&model.group_trail(0, &[0])), ["by-status"]);
    }

    #[test]
    fn a_view_group_can_be_the_root() {
        let notes = docs_notes();
        let views = [ViewDef::new("by-status", "status")];
        let options = SiteOptions {
            root: Some("views/by-status/open".to_string()),
            ..SiteOptions::default()
        };
        let model = model(&notes, &views, options);
        // Install carries an order, Aside does not.
        assert_eq!(sidebar_labels(&model), ["# open", "Install", "Aside"]);
    }

    #[test]
    fn a_single_tag_query_roots_the_sidebar_unless_a_root_is_configured() {
        let notes = docs_notes();
        let query = crate::query::parse("tag:Docs/Start").expect("parses");
        let model = Model::build_for(&notes, &[], &SiteOptions::default(), Some(&query), "V")
            .expect("model");
        assert_eq!(sidebar_labels(&model), ["# Getting Started", "Install"]);
        // Any other query leaves the sidebar alone.
        let query = crate::query::parse("tag:docs and status:open").expect("parses");
        let model = Model::build_for(&notes, &[], &SiteOptions::default(), Some(&query), "V")
            .expect("model");
        assert_eq!(sidebar_labels(&model)[0], "# tags");
        // A configured root wins over the query.
        let query = crate::query::parse("tag:docs/start").expect("parses");
        let options = SiteOptions {
            root: Some("tags/docs".to_string()),
            ..SiteOptions::default()
        };
        let model = Model::build_for(&notes, &[], &options, Some(&query), "V").expect("model");
        assert_eq!(sidebar_labels(&model)[0], "# Getting Started");
    }

    #[test]
    fn a_root_that_is_no_page_warns_and_keeps_the_default_sidebar() {
        let notes = docs_notes();
        for root in [
            "tags/nowhere",
            "views/no-such-view/x",
            "tags",
            "notes/basics",
            "views/by-status",
        ] {
            let options = SiteOptions {
                root: Some(root.to_string()),
                ..SiteOptions::default()
            };
            let model = model(&notes, &[ViewDef::new("by-status", "status")], options);
            assert_eq!(sidebar_labels(&model)[0], "# by-status", "{root}");
            assert_eq!(model.warnings.len(), 1, "{root}: {:?}", model.warnings);
            assert!(model.warnings[0].contains(root), "{}", model.warnings[0]);
        }
    }

    fn nav(toml: &str) -> SiteOptions {
        toml::from_str(toml).expect("the nav table parses")
    }

    #[test]
    fn the_nav_table_is_the_whole_sidebar() {
        let notes = docs_notes();
        let views = [ViewDef::new("by-status", "status")];
        let options = nav(&format!(
            "[[nav]]\nlabel = \"Guide\"\nitems = [\n  {{ note = \"{A}\" }},\n  {{ note = \"{B}\", label = \"Setting up\" }},\n  {{ label = \"Reference\", tag = \"docs/dev\" }},\n  {{ label = \"More\", items = [{{ view = \"by-status\", group = \"open\" }}, {{ tags = true }}] }},\n]\n[[nav]]\nlabel = \"Everything\"\nitems = [{{ view = \"by-status\" }}]\n"
        ));
        let model = model(&notes, &views, options);
        assert_eq!(
            sidebar_labels(&model),
            [
                "# Guide",
                "Getting Started",
                "Setting up",
                "[Reference]",
                "  Manifest",
                "<More>",
                "  [open]",
                "    Install",
                "    Aside",
                "  <tags>",
                "    [docs]",
                "      [Getting Started]",
                "        Install",
                "      [dev]",
                "        Manifest",
                "    [misc]",
                "      Aside",
                "# Everything",
                "<by-status>",
                "  [open]",
                "    Install",
                "    Aside",
            ]
        );
        assert!(model.warnings.is_empty(), "{:?}", model.warnings);
        assert_eq!(model.sidebar[0].page, None);
        // A curated step has no page in the breadcrumb; the first placement
        // wins, so Install's is the Guide section itself.
        let by_title = |title: &str| {
            model
                .notes
                .iter()
                .position(|n| n.title == title)
                .expect("the note exists")
        };
        let install = model.placements[by_title("Install")]
            .as_ref()
            .expect("placed");
        assert_eq!(labels(&install.trail), ["Guide"]);
        assert_eq!(install.trail[0].page, None);
        assert_eq!(install.prev, Some(by_title("Basics")));
        assert_eq!(install.next, Some(by_title("Manifest")));
        let aside = model.placements[by_title("Aside")]
            .as_ref()
            .expect("placed");
        assert_eq!(labels(&aside.trail), ["Guide", "More", "open"]);
        assert_eq!(aside.trail[1].page, None);
        assert_eq!(
            labels(&model.group_trail(1, &[0])),
            ["Guide", "More", "tags"]
        );
    }

    #[test]
    fn nav_items_that_name_nothing_the_site_has_warn_and_are_left_out() {
        let notes = docs_notes();
        let views = [ViewDef::new("by-status", "status")];
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        const Z: &str = "01ZRZ3NDEKTSV4RRFFQ69G5FAV";
        let options = nav(&format!(
            "[[nav]]\nlabel = \"Broken\"\nitems = [\n  {{ note = \"{Z}\" }},\n  {{ note = \"{E}\" }},\n  {{ tag = \"no-such-tag\" }},\n  {{ view = \"no-such-view\" }},\n  {{ view = \"by-status\", group = \"no-such-group\" }},\n  {{ items = [] }},\n  {{ note = \"{A}\", tag = \"docs\" }},\n  {{ label = \"x\" }},\n  {{ note = \"{C}\" }},\n]\n"
        ));
        let model = model(&notes, &views, options);
        assert_eq!(sidebar_labels(&model), ["# Broken", "Manifest"]);
        let joined = model.warnings.join("\n");
        assert_eq!(model.warnings.len(), 8, "{joined}");
        assert!(
            joined.contains(&format!("note {Z} is not among the exported notes")),
            "{joined}"
        );
        assert!(joined.contains(&format!("note {E} is hidden")), "{joined}");
        assert!(joined.contains("tag `no-such-tag`"), "{joined}");
        assert!(
            joined.contains("`no-such-view` is not a configured view"),
            "{joined}"
        );
        assert!(joined.contains("no group `no-such-group`"), "{joined}");
        assert!(joined.contains("needs a `label`"), "{joined}");
        assert!(joined.contains("only one of"), "{joined}");
        assert!(joined.contains("needs one of"), "{joined}");
    }

    #[test]
    fn related_notes_follow_the_site_setting_unless_a_note_overrides_it() {
        let notes = [
            note(A, "Plain", "tags: [t]\n"),
            note(B, "Off", "tags: [t]\nsite:\n  related: false\n"),
            note(C, "On", "tags: [t]\nsite:\n  related: true\n"),
        ];
        let by_title = |model: &Model, title: &str| {
            model
                .notes
                .iter()
                .position(|n| n.title == title)
                .expect("the note exists")
        };
        let on = model(&notes, &[], SiteOptions::default());
        assert!(on.shows_related(by_title(&on, "Plain")));
        assert!(!on.shows_related(by_title(&on, "Off")));
        assert!(on.shows_related(by_title(&on, "On")));
        let off = model(
            &notes,
            &[],
            SiteOptions {
                related: Some(false),
                ..SiteOptions::default()
            },
        );
        assert!(!off.shows_related(by_title(&off, "Plain")));
        assert!(!off.shows_related(by_title(&off, "Off")));
        assert!(off.shows_related(by_title(&off, "On")));
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
