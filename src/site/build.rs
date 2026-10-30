// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Building the site: every page and file the export writes, computed in
//! memory from the model and the converted notes (ADR 0046, ADR 0054).
//!
//! The build is separated from writing. [`build`] returns the complete set of
//! files with their contents plus the vault files to copy beside them, and
//! [`write`] puts them on disk; so the whole site is inspectable in a test
//! without touching a filesystem, and the one destructive step (emptying a
//! forced output directory) is the command's, not the library's.
//!
//! # Layout of the output
//!
//! ```text
//! index.html                       the front page
//! notes/<slug>.html                one page per note
//! tags/index.html                  the tag tree
//! tags/<a>/<b>/index.html          one page per tag
//! views/<name>/index.html          one index per configured view
//! views/<name>/<group>/index.html  one page per group
//! assets/                          the theme's files, style.css first,
//!                                  the scripts, and the search data
//! files/<vault path>               vault files the notes reference
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::id::Id;
use crate::link;
use crate::note::Note;
use crate::query::Query;
use crate::render::html::{self, Targets, frontmatter};
use crate::render::markdown::resolve_root_relative;
use crate::render::{LinkTarget, RenderError, ResolvedLink, join_within};
use crate::site::scope::ExportScope;
use crate::view::ViewDef;

use super::SiteOptions;
use super::frontend::{self, Grammars};
use super::model::{self, Front, Landing, Model, Placement};
use super::nav;
use super::page::{
    self, Crumb, NoteContext, NoteFragment, Page, PageKind, PageLink, TagLink, Templates,
};
use super::search;
use super::theme::{self, SiteTheme};

/// The directory of the theme's files within the site.
pub const ASSETS_DIR: &str = "assets";
/// The directory that mirrors the vault for the files notes reference.
pub const FILES_DIR: &str = "files";
/// The front page.
pub const INDEX_PAGE: &str = "index.html";

/// Everything the build needs.
pub struct Input<'a> {
    /// The notes to export, in any order.
    pub notes: &'a [Note],
    /// The ids of every note in the vault, exported or not, so a link to a
    /// note left out by the query can be told from a dangling one.
    pub vault_ids: &'a HashSet<Id>,
    pub views: &'a [ViewDef],
    pub options: &'a SiteOptions,
    /// The export's query, whose single `tag:` predicate roots the sidebar
    /// when the options name no root (ADR 0056).
    pub query: Option<&'a Query>,
    /// The site title when the options configure none.
    pub fallback_title: &'a str,
    pub vault_root: &'a Path,
    /// The vault theme with the directory its files live in, or `None` for
    /// the built-in theme.
    pub theme: Option<(&'a SiteTheme, &'a Path)>,
    /// The directory the site is written to, so a link to a directory
    /// holding it does not mirror the site into itself.
    pub output_dir: Option<&'a Path>,
}

/// One file the export writes, with its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// Site-relative path with `/` separators.
    pub path: String,
    pub contents: Vec<u8>,
}

/// One vault file the export copies beside the pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub from: PathBuf,
    /// Site-relative destination with `/` separators.
    pub to: String,
}

/// The built site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub files: Vec<OutputFile>,
    pub copies: Vec<Copy>,
    /// Export warnings: a referenced file that is missing or outside the
    /// vault, a link to a note outside the exported set, a configured index
    /// note that is not exported, a path an export never copies.
    pub warnings: Vec<String>,
    /// What the export left out as routine, such as the hidden entries and
    /// Finder metadata of a linked folder (ADR 0073). Reported, but never a
    /// warning.
    pub notes: Vec<String>,
}

impl Built {
    /// The file at `path`, for inspection.
    pub fn file(&self, path: &str) -> Option<&OutputFile> {
        self.files.iter().find(|file| file.path == path)
    }

    /// The number of HTML pages.
    pub fn page_count(&self) -> usize {
        self.files
            .iter()
            .filter(|file| file.path.ends_with(".html"))
            .count()
    }
}

/// Build the whole site in memory.
pub fn build(input: &Input<'_>) -> Result<Built, crate::error::Error> {
    let model = Model::build_for(
        input.notes,
        input.views,
        input.options,
        input.query,
        input.fallback_title,
    )?;
    let mut warnings = model.warnings.clone();
    let mut notes = Vec::new();
    let mut files = Vec::new();
    let mut copies = Vec::new();
    let excluded: Vec<&Path> = input.output_dir.into_iter().collect();
    let scope = ExportScope::new(input.vault_root, &excluded);
    // Every page inlines the selected theme's icon sprite and renders with
    // its templates, parsed once here.
    let theme = SiteTheme::selected(input.theme.map(|(theme, _)| theme));
    let icons = theme.sprite();
    let templates = Templates::new(&theme)?;
    let export = Export {
        model: &model,
        templates: &templates,
        icons: &icons,
        vars: &input.options.vars,
        scope: &scope,
    };

    // The grammars every page's code blocks need, written once under
    // `assets/grammars/` beside the page script.
    let grammars = frontend::grammars()?;
    let mut used_grammars: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Links resolve against the exported set only, so a link to a note the
    // query left out renders as its display text rather than pointing at a
    // page that does not exist.
    let by_id: HashMap<Id, &Note> = input.notes.iter().map(|note| (note.id, note)).collect();

    // Sort the way the model did, so note index and page line up.
    let mut ordered: Vec<&Note> = input.notes.iter().collect();
    ordered.sort_by_key(|note| std::cmp::Reverse(note.id));

    // Every referenced vault file, root-absolute, each once.
    let mut referenced: Vec<String> = Vec::new();

    // A landing note (ADR 0056) has no page of its own: it is rendered as
    // the page of every group it lands, above that group's listing.
    let landings = model.landings();

    for (index, note) in ordered.iter().enumerate() {
        let entry = &model.notes[index];
        let links = resolve_links(note, &by_id, input.vault_ids, &mut warnings);
        let note_dir = root_absolute_dir(input.vault_root, &note.path);

        let landed: Vec<&Landing> = landings
            .iter()
            .filter(|landing| landing.note == index)
            .collect();
        let pages: Vec<(String, Option<GroupPage>)> = if landed.is_empty() {
            vec![(entry.page.clone(), None)]
        } else {
            landed
                .iter()
                .map(|landing| {
                    // The landing note stands in for the listing unless it
                    // asks for it.
                    let group = model.group_at(landing.section, &landing.path);
                    let mut page = group_page(&model, landing.section, &landing.path);
                    if !entry.site.listing {
                        page.listing.clear();
                    }
                    (group.page.clone(), Some(page))
                })
                .collect()
        };

        let mut rendered_assets: Vec<String> = Vec::new();
        for (at, (page, extra)) in pages.into_iter().enumerate() {
            // The unknown-language warnings are raised once, by the first
            // page of the note; a second rendering raises none twice.
            let mut repeated = Vec::new();
            let rendered = render_note(
                &export,
                index,
                note,
                &links,
                &note_dir,
                &page,
                &grammars,
                if at == 0 {
                    &mut warnings
                } else {
                    &mut repeated
                },
                extra,
            )?;
            files.push(OutputFile {
                path: page,
                contents: rendered.page.into_bytes(),
            });
            used_grammars.extend(rendered.grammars);
            rendered_assets = rendered.assets;
        }
        for dest in rendered_assets {
            match resolve_root_relative(&note_dir, &dest) {
                Some(path) => {
                    if !referenced.contains(&path) {
                        referenced.push(path);
                    }
                }
                None => warnings.push(format!(
                    "{}: `{dest}` points outside the vault and is not copied",
                    note.path.display()
                )),
            }
        }

        if model.front == Front::Note(index) {
            // The unknown-language warnings were raised by the note's own
            // page; the front-page copy raises none twice.
            let mut repeated = Vec::new();
            let rendered = render_note(
                &export,
                index,
                note,
                &links,
                &note_dir,
                INDEX_PAGE,
                &grammars,
                &mut repeated,
                None,
            )?;
            files.push(OutputFile {
                path: INDEX_PAGE.to_string(),
                contents: rendered.page.into_bytes(),
            });
        }
    }

    files.push(OutputFile {
        path: format!("{ASSETS_DIR}/{}", frontend::APP_SCRIPT),
        contents: frontend::app_js().as_bytes().to_vec(),
    });
    files.push(OutputFile {
        path: format!("{ASSETS_DIR}/{}", frontend::SEARCH_SCRIPT),
        contents: frontend::search_js().as_bytes().to_vec(),
    });
    // The search data covers the exported set; the page script loads it
    // the first time a reader opens the search.
    files.push(OutputFile {
        path: format!("{ASSETS_DIR}/{}", search::SEARCH_DATA),
        contents: search::script(&model, &ordered).into_bytes(),
    });
    for name in &used_grammars {
        let grammar = grammars
            .get(name)
            .expect("used grammar names come from the blob");
        files.push(OutputFile {
            path: format!(
                "{ASSETS_DIR}/{}/{}",
                frontend::GRAMMARS_DIR,
                grammar.file_name()
            ),
            contents: grammar.script().into_bytes(),
        });
    }

    if model.front == Front::Overview {
        let body = nav::overview(&model, "");
        let page = chrome(
            &export,
            INDEX_PAGE,
            &model.title,
            PageKind::Front,
            body,
            Vec::new(),
            String::new(),
            None,
            None,
            &[],
        );
        files.push(OutputFile {
            path: INDEX_PAGE.to_string(),
            contents: page.render(&templates)?.into_bytes(),
        });
    }

    // Section indexes and group pages.
    for (section_index, section) in model.sections.iter().enumerate() {
        let prefix = nav::prefix_for(&section.page);
        let body = format!(
            "<h1>{}</h1>\n{}",
            html::writer::escape(&section.title),
            nav::group_list(&section.groups, &prefix)
        );
        let page = chrome(
            &export,
            &section.page,
            &section.title,
            PageKind::Group,
            body,
            Vec::new(),
            String::new(),
            None,
            None,
            &[],
        );
        files.push(OutputFile {
            path: section.page.clone(),
            contents: page.render(&templates)?.into_bytes(),
        });

        // Depth-first over the groups by path; a group with a landing note
        // got its page in the notes loop.
        let mut stack: Vec<(&model::Group, Vec<usize>)> = section
            .groups
            .iter()
            .enumerate()
            .map(|(position, group)| (group, vec![position]))
            .collect();
        while let Some((group, path)) = stack.pop() {
            for (position, child) in group.children.iter().enumerate() {
                stack.push((child, [path.as_slice(), &[position]].concat()));
            }
            if group.landing.is_some() {
                continue;
            }
            let extra = group_page(&model, section_index, &path);
            let body = format!(
                "<h1>{}</h1>\n{}",
                html::writer::escape(&group.value),
                extra.listing
            );
            let page = chrome(
                &export,
                &group.page,
                &group.value,
                PageKind::Group,
                body,
                extra.crumbs,
                String::new(),
                None,
                None,
                &[],
            );
            files.push(OutputFile {
                path: group.page.clone(),
                contents: page.render(&templates)?.into_bytes(),
            });
        }
    }

    // The theme's files under `assets/`, the stylesheet among them; the
    // templates were rendered with and are not served. A theme directory is
    // copied by the rules of a linked one (ADR 0073), so a theme cloned with
    // its repository does not publish it.
    match input.theme {
        Some((theme, dir)) => {
            let walk = ExportScope::for_theme(dir, input.vault_root).walk(dir)?;
            let subject = "theme";
            warnings.extend(walk.warning(subject, &theme.name));
            notes.extend(walk.note(subject, &theme.name));
            for (relative, from) in walk.files {
                if theme::is_template(&relative) {
                    continue;
                }
                copies.push(Copy {
                    from,
                    to: format!("{ASSETS_DIR}/{relative}"),
                });
            }
        }
        None => {
            for asset in theme::BUILTIN_FILES {
                if theme::is_template(asset.path) {
                    continue;
                }
                files.push(OutputFile {
                    path: format!("{ASSETS_DIR}/{}", asset.path),
                    contents: asset.contents(),
                });
            }
        }
    }

    let mirror = mirrored(&scope, &referenced)?;
    copies.extend(mirror.copies);
    warnings.extend(mirror.warnings);
    notes.extend(mirror.notes);

    Ok(Built {
        files,
        copies,
        warnings,
        notes,
    })
}

/// The vault files referenced paths bring into an export, and what the
/// export reports about them.
#[derive(Debug, Default)]
pub(crate) struct Mirrored {
    pub copies: Vec<Copy>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

/// The copies that mirror `referenced`, root-absolute vault paths, under
/// `files/`. A linked directory is mirrored with its tree, so the link
/// resolves in the site the way it does in the vault, without what the
/// scope leaves out (ADR 0073). A path that does not exist, one that is
/// refused, and a directory that has nothing to copy are warnings; routine
/// metadata left out of a directory is a note. Each target is copied once,
/// however many links reach it.
pub(crate) fn mirrored(
    scope: &ExportScope,
    referenced: &[String],
) -> Result<Mirrored, crate::fsutil::FsError> {
    let mut mirror = Mirrored::default();
    let mut targets: HashSet<String> = HashSet::new();
    let mut copy = |mirror: &mut Mirrored, from: PathBuf, to: String| {
        if targets.insert(to.clone()) {
            mirror.copies.push(Copy { from, to });
        }
    };
    for path in referenced {
        let from = scope.vault_path(path);
        if let Err(refusal) = scope.verdict(&from) {
            mirror
                .warnings
                .push(refusal.message("referenced path", path));
        } else if from.is_file() {
            copy(&mut mirror, from, format!("{FILES_DIR}{path}"));
        } else if from.is_dir() {
            let walk = scope.walk(&from)?;
            let subject = "referenced directory";
            if walk.files.is_empty() {
                mirror
                    .warnings
                    .push(format!("{subject} `{path}` has nothing to copy"));
            }
            mirror.warnings.extend(walk.warning(subject, path));
            mirror.notes.extend(walk.note(subject, path));
            let base = format!("{FILES_DIR}{}", path.trim_end_matches('/'));
            for (relative, file) in walk.files {
                copy(&mut mirror, file, format!("{base}/{relative}"));
            }
        } else {
            mirror.warnings.push(format!(
                "referenced file not found in the vault: {}",
                from.display()
            ));
        }
    }
    Ok(mirror)
}

/// Write a built site into `dir`, creating directories as needed.
///
/// Every target must lie inside `dir` (ADR 0072). All of them are checked
/// before the first file is written, so a site with one target outside
/// `dir` fails without writing any file.
pub fn write(built: &Built, dir: &Path) -> std::io::Result<()> {
    let inside = |relative: &str| {
        join_within(dir, relative).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("`{relative}` would be written outside the output directory"),
            )
        })
    };
    let files = built
        .files
        .iter()
        .map(|file| Ok((inside(&file.path)?, &file.contents)))
        .collect::<std::io::Result<Vec<_>>>()?;
    let copies = built
        .copies
        .iter()
        .map(|copy| Ok((inside(&copy.to)?, &copy.from)))
        .collect::<std::io::Result<Vec<_>>>()?;

    for (path, contents) in files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    for (path, from) in copies {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, path)?;
    }
    Ok(())
}

/// A note's links resolved against the exported set. A target that exists in
/// the vault but is not exported is a warning and stays unresolved.
fn resolve_links(
    note: &Note,
    exported: &HashMap<Id, &Note>,
    vault_ids: &HashSet<Id>,
    warnings: &mut Vec<String>,
) -> Vec<ResolvedLink> {
    link::extract(&note.body)
        .into_iter()
        .map(|found| {
            let target = exported.get(&found.id).map(|target| LinkTarget {
                title: target.title.clone(),
                slug: target.slug.clone(),
            });
            if target.is_none() && vault_ids.contains(&found.id) {
                warnings.push(format!(
                    "{}: links to {}, which is not among the exported notes",
                    note.path.display(),
                    found.id
                ));
            }
            ResolvedLink {
                range: found.range,
                display: found.display.to_string(),
                id: found.id,
                target,
            }
        })
        .collect()
}

/// The note's directory as a root-absolute path within the vault, `/all-notes`
/// for a standard layout.
pub(crate) fn root_absolute_dir(vault_root: &Path, note_path: &Path) -> String {
    let dir = note_path.parent().unwrap_or(vault_root);
    match dir.strip_prefix(vault_root) {
        Ok(relative) if !relative.as_os_str().is_empty() => {
            format!("/{}", relative.to_string_lossy().replace('\\', "/"))
        }
        _ => "/".to_string(),
    }
}

/// Where a note page's links point: note links at the target's page,
/// vault files at their mirror under `files/`, both relative to the page.
/// A path the export does not copy keeps its text.
struct PageTargets<'a> {
    model: &'a Model,
    prefix: String,
    note_dir: &'a str,
    scope: &'a ExportScope,
}

impl Targets for PageTargets<'_> {
    fn note_href(&self, id: Id, slug: &str) -> String {
        match self.model.page_of(id) {
            Some(page) => format!("{}{page}", self.prefix),
            None => format!("{}notes/{slug}.html", self.prefix),
        }
    }

    fn asset_href(&self, dest: &str) -> String {
        match self.scope.href_path(self.note_dir, dest) {
            Some(path) => format!("{}{FILES_DIR}{path}", self.prefix),
            None => dest.to_string(),
        }
    }
}

struct RenderedNote {
    page: String,
    /// Local paths the body references, as written.
    assets: Vec<String>,
    /// The grammars the page's code blocks need, their embedded grammars
    /// included, by name.
    grammars: Vec<String>,
}

/// What a group's page carries besides a heading or a landing note: the
/// listing of its child groups and its notes, and its breadcrumb.
struct GroupPage {
    listing: String,
    crumbs: Vec<Crumb>,
}

/// The listing and breadcrumb of the group at `path` in the section.
fn group_page(model: &Model, section: usize, path: &[usize]) -> GroupPage {
    let group = model.group_at(section, path);
    let prefix = nav::prefix_for(&group.page);
    let crumbs = crumbs(&model.group_trail(section, path), &prefix);
    GroupPage {
        listing: format!(
            "{}{}",
            nav::group_list(&group.children, &prefix),
            nav::listing(model, &group.descendants(), &prefix)
        ),
        crumbs,
    }
}

/// Render one note as a site page at `at`: its own page, `index.html` when
/// it is the front page, or a group's page when it is that group's landing
/// note, in which case `group` supplies the listing that follows the body
/// and the breadcrumb (an empty listing when the note stands alone), and
/// the related notes are left out; they are also left out where the site or
/// the note switches them off. A fence language
/// without a grammar is a warning; its block stays plain.
///
/// Every argument is a distinct input of the page; bundling them into a
/// struct would name the same things one level down.
#[allow(clippy::too_many_arguments)]
fn render_note(
    export: &Export<'_>,
    index: usize,
    note: &Note,
    links: &[ResolvedLink],
    note_dir: &str,
    at: &str,
    grammars: &Grammars,
    warnings: &mut Vec<String>,
    group: Option<GroupPage>,
) -> Result<RenderedNote, RenderError> {
    let model = export.model;
    let prefix = nav::prefix_for(at);
    let targets = PageTargets {
        model,
        prefix: prefix.clone(),
        note_dir,
        scope: export.scope,
    };
    let emitted = html::emit(&note.body, links, &targets, Some(&note.title));
    let entry = &model.notes[index];

    let mut wanted: Vec<&str> = Vec::new();
    for language in &emitted.languages {
        match grammars.resolve(language) {
            Some(grammar) => wanted.push(grammar.name.as_str()),
            None => warnings.push(format!(
                "{}: no highlighting grammar for `{language}`; the block stays plain",
                note.path.display()
            )),
        }
    }
    let needed: Vec<String> = grammars
        .closure(wanted)
        .into_iter()
        .map(|grammar| grammar.name.clone())
        .collect();

    let tags = entry
        .tags
        .iter()
        .map(|tag| TagLink {
            label: tag.clone(),
            href: Some(format!("{prefix}tags/{tag}/index.html")),
        })
        .collect();
    let mut body = NoteFragment {
        title: entry.title.clone(),
        created: entry.created.clone(),
        tags,
        fields: frontmatter::fields(&note.frontmatter),
        body: emitted.html,
    }
    .render(export.templates)?;

    // What the page is: a landing note's page is its group's, the index
    // note's copy at the root is the front page, anything else the note's
    // own page.
    let kind = match (&group, at) {
        (Some(_), _) => PageKind::Group,
        (None, INDEX_PAGE) => PageKind::Front,
        (None, _) => PageKind::Note,
    };
    let placement = model.placements[index].as_ref();
    let crumbs = match group {
        Some(group) => {
            body.push_str(&group.listing);
            group.crumbs
        }
        None => {
            if model.shows_related(index) {
                body.push_str(&nav::related(model, &model.related(index), &prefix));
            }
            placement
                .map(|placement| breadcrumbs(placement, &prefix))
                .unwrap_or_default()
        }
    };
    let neighbour = |neighbour: Option<usize>| {
        neighbour.map(|other| PageLink {
            label: model.notes[other].name().to_string(),
            href: format!("{prefix}{}", model.notes[other].page),
        })
    };
    let prev = placement.and_then(|placement| neighbour(placement.prev));
    let next = placement.and_then(|placement| neighbour(placement.next));

    let mut page = chrome(
        export,
        at,
        &entry.title,
        kind,
        body,
        crumbs,
        nav::outline(&emitted.headings),
        prev,
        next,
        &needed,
    );
    page.note = Some(NoteContext {
        id: note.id.to_string(),
        title: entry.title.clone(),
        created: entry.created.clone(),
        tags: entry.tags.clone(),
        frontmatter: note.frontmatter.clone(),
    });
    // The note may name its own template; one the theme does not have is
    // a warning, and the page keeps the default.
    if let Some(name) = &entry.site.template {
        let template = page::template_name(name);
        if export.templates.has(&template) {
            page.template = template;
        } else {
            warnings.push(format!(
                "{}: `site.template` names `{name}`, which the theme has no template for; `{}` is used",
                note.path.display(),
                page::PAGE_TEMPLATE
            ));
        }
    }
    Ok(RenderedNote {
        page: page.render(export.templates)?,
        assets: emitted.assets,
        grammars: needed,
    })
}

/// The breadcrumb of a placed note, relative to its page.
fn breadcrumbs(placement: &Placement, prefix: &str) -> Vec<Crumb> {
    crumbs(&placement.trail, prefix)
}

/// A model trail as the page's breadcrumb, each step's page made relative
/// to the page rendered.
fn crumbs(trail: &[model::Crumb], prefix: &str) -> Vec<Crumb> {
    trail
        .iter()
        .map(|step| Crumb {
            label: step.label.clone(),
            href: step.page.as_ref().map(|page| format!("{prefix}{page}")),
        })
        .collect()
}

/// What every page of one export shares: the model, the theme's templates
/// and icon sprite, the config's `[site.vars]`, and what the export may copy.
struct Export<'a> {
    model: &'a Model,
    templates: &'a Templates,
    icons: &'a str,
    vars: &'a toml::Table,
    scope: &'a ExportScope,
}

/// The chrome of a page at `at`: sidebar with the current trail open, the
/// stylesheet and scripts relative to the page (one script per grammar in
/// `grammars`, then the page script and the search script), and the given
/// navigation. The page is rendered from no note; a caller that has one
/// sets `note` afterwards.
#[allow(clippy::too_many_arguments)]
fn chrome(
    export: &Export<'_>,
    at: &str,
    title: &str,
    kind: PageKind,
    body: String,
    breadcrumbs: Vec<Crumb>,
    outline: String,
    prev: Option<PageLink>,
    next: Option<PageLink>,
    grammars: &[String],
) -> Page {
    let prefix = nav::prefix_for(at);
    // Deferred scripts run in document order, and the page script highlights
    // as soon as it runs, so every grammar script has to register before it.
    let mut scripts: Vec<String> = grammars
        .iter()
        .map(|grammar| {
            format!(
                "{prefix}{ASSETS_DIR}/{}/{grammar}.js",
                frontend::GRAMMARS_DIR
            )
        })
        .collect();
    scripts.push(format!("{prefix}{ASSETS_DIR}/{}", frontend::APP_SCRIPT));
    scripts.push(format!("{prefix}{ASSETS_DIR}/{}", frontend::SEARCH_SCRIPT));
    let model = export.model;
    Page {
        lang: model.lang.clone(),
        site_title: model.title.clone(),
        title: title.to_string(),
        path: at.to_string(),
        kind,
        note: None,
        vars: export.vars.clone(),
        nav: nav::sidebar_data(model, &prefix, at),
        template: page::PAGE_TEMPLATE.to_string(),
        stylesheet: format!("{prefix}{ASSETS_DIR}/{}", theme::STYLESHEET),
        scripts,
        icons: export.icons.to_string(),
        sidebar: nav::sidebar(model, &prefix, at),
        prefix,
        breadcrumbs,
        outline,
        prev,
        next,
        body,
    }
}

/// Every file under `dir`, recursively, as `(relative path, absolute path)`,
/// sorted, with `/` separators in the relative part.
pub(crate) fn files_under(dir: &Path) -> Result<Vec<(String, PathBuf)>, crate::fsutil::FsError> {
    fn walk(
        base: &Path,
        dir: &Path,
        out: &mut Vec<(String, PathBuf)>,
    ) -> Result<(), crate::fsutil::FsError> {
        for (path, file_type) in crate::fsutil::read_dir_entries(dir)? {
            if file_type.is_dir() {
                walk(base, &path, out)?;
            } else {
                let relative = path
                    .strip_prefix(base)
                    .expect("walked paths lie under the base")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((relative, path));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::slug;

    const A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";
    const C: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";
    /// In the vault but not exported.
    const D: &str = "01DRZ3NDEKTSV4RRFFQ69G5FAV";

    /// A vault directory holding a diagram beside the notes, a shared
    /// asset outside `all-notes/`, a gallery directory with a nested file,
    /// and an empty directory.
    fn vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("all-notes/assets/gallery/sub")).expect("gallery");
        std::fs::create_dir_all(dir.path().join("all-notes/assets/empty")).expect("empty");
        std::fs::create_dir_all(dir.path().join("assets")).expect("assets");
        std::fs::write(dir.path().join("all-notes/diagram.png"), b"png").expect("diagram");
        std::fs::write(dir.path().join("all-notes/assets/gallery/a.jpg"), b"a").expect("a");
        std::fs::write(dir.path().join("all-notes/assets/gallery/sub/b.jpg"), b"b").expect("b");
        std::fs::write(dir.path().join("assets/logo.svg"), b"svg").expect("logo");
        dir
    }

    fn note(vault: &Path, id: &str, title: &str, frontmatter: &str, body: &str) -> Note {
        let content = format!("---\ntitle: {title}\n{frontmatter}---\n{body}");
        Note::parse(
            vault.join(format!("all-notes/{id}-{}.md", slug::slugify(title))),
            &content,
            None,
        )
        .expect("the fixture note parses")
    }

    fn notes(vault: &Path) -> Vec<Note> {
        vec![
            note(
                vault,
                A,
                "Rust Tips",
                "tags: [programming/rust]\nstatus: done\n",
                &format!(
                    "## Intro\n\nSee [the plain one]({B}-plain.md) and [gone]({D}-gone.md).\n\n![d](diagram.png) ![l](../assets/logo.svg) ![o](../../outside.png) ![m](missing.png)\n\n[gallery](assets/gallery) [empty](assets/empty)\n\n```rust\nfn x() {{}}\n```\n\n```cobol\nDISPLAY.\n```\n"
                ),
            ),
            note(vault, B, "Plain", "status: open\n", "Nothing much.\n"),
            note(vault, C, "Untagged", "", "Loose.\n"),
        ]
    }

    fn ids(notes: &[Note]) -> HashSet<Id> {
        let mut ids: HashSet<Id> = notes.iter().map(|n| n.id).collect();
        ids.insert(D.parse().expect("ulid"));
        ids
    }

    fn build_site(vault: &Path, options: &SiteOptions) -> Built {
        let notes = notes(vault);
        let vault_ids = ids(&notes);
        let views = [ViewDef::parse("by-status", "status").expect("valid view name")];
        build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &views,
            options,
            query: None,
            fallback_title: "Vault",
            vault_root: vault,
            theme: None,
            output_dir: None,
        })
        .expect("the site builds")
    }

    fn text(built: &Built, path: &str) -> String {
        String::from_utf8(built.file(path).expect(path).contents.clone()).expect("utf-8")
    }

    #[test]
    fn the_file_set_pins_the_site_layout() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let mut paths: Vec<&str> = built.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        let mut copies: Vec<String> = built
            .copies
            .iter()
            .map(|c| {
                format!(
                    "{} <- {}",
                    c.to,
                    c.from
                        .strip_prefix(vault.path())
                        .expect("in vault")
                        .display()
                )
            })
            .collect();
        copies.sort();
        insta::assert_snapshot!(format!("{}\n--\n{}", paths.join("\n"), copies.join("\n")));
        assert_eq!(built.page_count(), 10);
    }

    #[test]
    fn warnings_cover_excluded_links_and_bad_references() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let joined = built.warnings.join("\n");
        assert!(joined.contains(&format!("links to {D}")), "{joined}");
        assert!(
            joined.contains("`../../outside.png` points outside the vault"),
            "{joined}"
        );
        assert!(
            joined.contains("referenced file not found in the vault")
                && joined.contains("missing.png"),
            "{joined}"
        );
        assert!(
            joined.contains("no highlighting grammar for `cobol`"),
            "{joined}"
        );
        assert!(
            joined.contains("referenced directory `/all-notes/assets/empty` has nothing to copy"),
            "{joined}"
        );
        assert_eq!(built.warnings.len(), 5, "{joined}");
    }

    #[test]
    fn a_linked_directory_is_mirrored_with_its_tree() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let page = text(&built, "notes/rust-tips.html");
        assert!(
            page.contains("<a href=\"../files/all-notes/assets/gallery\">gallery</a>"),
            "{page}"
        );
        let mut gallery: Vec<&str> = built
            .copies
            .iter()
            .filter(|c| c.to.starts_with("files/all-notes/assets/gallery/"))
            .map(|c| c.to.as_str())
            .collect();
        gallery.sort();
        assert_eq!(
            gallery,
            [
                "files/all-notes/assets/gallery/a.jpg",
                "files/all-notes/assets/gallery/sub/b.jpg",
            ]
        );
        assert!(
            !built.warnings.iter().any(|w| w.contains("gallery")),
            "{}",
            built.warnings.join("\n")
        );
    }

    #[test]
    fn a_note_page_links_relative_to_itself() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let page = text(&built, "notes/rust-tips.html");
        assert!(
            page.contains("<link rel=\"stylesheet\" href=\"../assets/style.css\">"),
            "{page}"
        );
        assert!(
            page.contains("<a class=\"note-link\" href=\"../notes/plain.html\">Plain</a>"),
            "{page}"
        );
        // The excluded target renders as its display text.
        assert!(page.contains("and gone."), "{page}");
        assert!(
            page.contains("<img src=\"../files/all-notes/diagram.png\" alt=\"d\">"),
            "{page}"
        );
        assert!(
            page.contains("<img src=\"../files/assets/logo.svg\" alt=\"l\">"),
            "{page}"
        );
        assert!(
            page.contains("<img src=\"../../outside.png\" alt=\"o\">"),
            "{page}"
        );
        assert!(
            page.contains("<li class=\"depth-2\"><a href=\"#intro\">Intro</a></li>"),
            "{page}"
        );
        // Breadcrumb from the first placement: the view section, then the
        // group; the sidebar opens that group.
        assert!(
            page.contains("<use href=\"#icon-chevron-right\"/></svg><a href=\"../views/by-status/done/index.html\">done</a></li>"),
            "{page}"
        );
        assert!(
            page.contains(
                "<details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../views/by-status/done/index.html\">done</a>"
            ),
            "{page}"
        );
    }

    #[test]
    fn pages_load_the_page_script_and_only_their_grammars() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let page = text(&built, "notes/rust-tips.html");
        assert!(
            page.contains("<script defer src=\"../assets/app.js\"></script>"),
            "{page}"
        );
        assert!(
            page.contains("<script defer src=\"../assets/grammars/rust.js\"></script>"),
            "{page}"
        );
        // The grammar registers before the page script highlights: deferred
        // scripts run in document order.
        let grammar_at = page.find("grammars/rust.js").expect("grammar script");
        let app_at = page.find("assets/app.js").expect("page script");
        assert!(grammar_at < app_at, "{page}");
        assert!(!page.contains("grammars/cobol.js"), "{page}");
        // The search script follows the page script; a page without code
        // loads those two alone.
        assert!(
            page.contains("<script defer src=\"../assets/app.js\"></script>\n<script defer src=\"../assets/search.js\"></script>"),
            "{page}"
        );
        let plain = text(&built, "notes/plain.html");
        assert!(plain.contains("assets/app.js"), "{plain}");
        assert!(plain.contains("assets/search.js"), "{plain}");
        assert!(!plain.contains("assets/grammars/"), "{plain}");
        assert_eq!(text(&built, "assets/search.js"), frontend::search_js());
        // The used grammar is written once, as a registering script; the
        // page script is the embedded one.
        let script = text(&built, "assets/grammars/rust.js");
        assert!(script.contains("w.__ntropyGrammars.push({"), "{script}");
        assert!(script.contains("\"name\":\"rust\""), "{script}");
        assert!(built.file("assets/grammars/cobol.js").is_none());
        assert_eq!(text(&built, "assets/app.js"), frontend::app_js());
    }

    #[test]
    fn every_page_mounts_the_search_over_the_embedded_data() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let page = text(&built, "notes/rust-tips.html");
        assert!(
            page.contains(
                "<button class=\"search-toggle\" type=\"button\" aria-haspopup=\"dialog\" data-search=\"../assets/search-data.js\" data-prefix=\"../\">"
            ),
            "{page}"
        );
        let index = text(&built, "index.html");
        assert!(
            index.contains("data-search=\"assets/search-data.js\" data-prefix=\"\">"),
            "{index}"
        );
        // The data lists the exported notes newest first, with the page
        // each entry links to.
        let data = text(&built, "assets/search-data.js");
        assert!(data.starts_with("window.__ntropySearch="), "{data}");
        let json: serde_json::Value = serde_json::from_str(
            data.trim_start_matches("window.__ntropySearch=")
                .trim_end_matches(";\n"),
        )
        .expect("the payload is JSON");
        let pages: Vec<&str> = json["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .map(|entry| entry["page"].as_str().expect("page"))
            .collect();
        assert_eq!(
            pages,
            [
                "notes/untagged.html",
                "notes/plain.html",
                "notes/rust-tips.html"
            ]
        );
        assert_eq!(json["notes"][2]["frontmatter"]["status"], "done");
        // The site's own pages ride along, so a search finds a tag page.
        let tag_pages: Vec<&str> = json["pages"]
            .as_array()
            .expect("pages")
            .iter()
            .filter(|p| p["kind"] == "tag")
            .map(|p| p["value"].as_str().expect("value"))
            .collect();
        assert_eq!(tag_pages, ["programming", "programming/rust"]);
    }

    #[test]
    fn neighbours_follow_the_reading_order_of_the_first_section() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        // The view section reads `done` (Rust Tips) then `open` (Plain):
        // the pager crosses the group boundary.
        let page = text(&built, "notes/plain.html");
        assert!(
            page.contains("<a class=\"prev\" rel=\"prev\" href=\"../notes/rust-tips.html\">"),
            "{page}"
        );
        assert!(!page.contains("class=\"next\""), "{page}");
        // A note in no group has neither breadcrumb nor pager.
        let loose = text(&built, "notes/untagged.html");
        assert!(!loose.contains("class=\"breadcrumbs\""), "{loose}");
        assert!(!loose.contains("class=\"pager\""), "{loose}");
    }

    #[test]
    fn the_front_page_is_an_overview_or_the_configured_note() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let index = text(&built, "index.html");
        assert!(
            index.contains("<h1 class=\"site-title\">Vault</h1>"),
            "{index}"
        );
        assert!(index.contains("3 notes"), "{index}");
        assert!(
            index.contains("<link rel=\"stylesheet\" href=\"assets/style.css\">"),
            "{index}"
        );
        assert!(
            index.contains("<a href=\"notes/plain.html\">Plain</a>"),
            "{index}"
        );

        let configured = SiteOptions {
            index: Some(A.to_string()),
            title: Some("Docs".to_string()),
            ..SiteOptions::default()
        };
        let built = build_site(vault.path(), &configured);
        let index = text(&built, "index.html");
        assert!(
            index.contains("<h1 class=\"note-title\">Rust Tips</h1>"),
            "{index}"
        );
        // Links from the root-level copy point at `notes/` and `files/`
        // without climbing.
        assert!(
            index.contains("<a class=\"note-link\" href=\"notes/plain.html\">Plain</a>"),
            "{index}"
        );
        assert!(
            index.contains("<img src=\"files/all-notes/diagram.png\""),
            "{index}"
        );
        assert!(index.contains("<title>Rust Tips · Docs</title>"), "{index}");
    }

    #[test]
    fn group_and_section_pages_list_children_and_descendants() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let tags = text(&built, "tags/index.html");
        assert!(
            tags.contains(
                "<a class=\"chip\" href=\"../tags/programming/index.html\">programming<span class=\"count\">1</span></a>"
            ),
            "{tags}"
        );
        let programming = text(&built, "tags/programming/index.html");
        assert!(
            programming.contains("<h1>programming</h1>"),
            "{programming}"
        );
        assert!(
            programming.contains(
                "<a class=\"chip\" href=\"../../tags/programming/rust/index.html\">rust<span class=\"count\">1</span></a>"
            ),
            "{programming}"
        );
        assert!(
            programming.contains(
                "<a class=\"note-row-title\" href=\"../../notes/rust-tips.html\">Rust Tips</a>"
            ),
            "{programming}"
        );
        let rust = text(&built, "tags/programming/rust/index.html");
        assert!(
            rust.contains("<li><a href=\"../../../tags/index.html\">tags</a></li>"),
            "{rust}"
        );
        assert!(
            rust.contains(
                "<use href=\"#icon-chevron-right\"/></svg><a href=\"../../../tags/programming/index.html\">programming</a></li>"
            ),
            "{rust}"
        );
    }

    #[test]
    fn a_vault_theme_is_copied_under_assets() {
        let vault = vault();
        let theme_dir = vault.path().join(".ntropy/themes/site/corporate");
        std::fs::create_dir_all(theme_dir.join("fonts")).expect("theme dirs");
        std::fs::write(theme_dir.join("style.css"), "body{}").expect("css");
        std::fs::write(theme_dir.join("fonts/a.woff2"), b"font").expect("font");
        std::fs::create_dir_all(theme_dir.join("templates")).expect("templates dir");
        std::fs::write(
            theme_dir.join("templates/page.html"),
            "{% extends \"ntropy/page.html\" %}{% block body_class %}themed{% endblock %}",
        )
        .expect("template");
        let theme = SiteTheme {
            name: "corporate".to_string(),
            stylesheet: "body{}".to_string(),
            icons: std::collections::BTreeMap::from([(
                "x".to_string(),
                "<symbol id=\"icon-x\"/>".to_string(),
            )]),
            templates: std::collections::BTreeMap::from([(
                "page.html".to_string(),
                "{% extends \"ntropy/page.html\" %}{% block body_class %}themed{% endblock %}"
                    .to_string(),
            )]),
        };
        let notes = notes(vault.path());
        let vault_ids = ids(&notes);
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: Some((&theme, &theme_dir)),
            output_dir: None,
        })
        .expect("builds");
        let theme_copies: Vec<&str> = built
            .copies
            .iter()
            .filter(|c| c.to.starts_with("assets/"))
            .map(|c| c.to.as_str())
            .collect();
        assert_eq!(
            theme_copies,
            ["assets/fonts/a.woff2", "assets/style.css"],
            "the templates are not served"
        );
        assert!(
            built.file("assets/style.css").is_none(),
            "the built-in stylesheet is not written"
        );
        // The pages inline the theme's own sprite and render with its
        // template, every page kind alike.
        let page = text(&built, "notes/rust-tips.html");
        assert!(
            page.contains("aria-hidden=\"true\"><symbol id=\"icon-x\"/></svg>"),
            "{page}"
        );
        assert!(!page.contains("<symbol id=\"icon-menu\""), "{page}");
        for path in ["notes/rust-tips.html", "index.html", "tags/index.html"] {
            assert!(
                text(&built, path).contains("<body class=\"themed\">"),
                "{path} is rendered with the theme's template"
            );
        }
    }

    /// Every page tells the template what it is rendered from. The probe
    /// template prints the context instead of a page.
    #[test]
    fn every_page_tells_its_kind_and_carries_its_note_and_the_vars() {
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        let vault = vault();
        let theme_dir = vault.path().join(".ntropy/themes/site/probe");
        std::fs::create_dir_all(&theme_dir).expect("theme dir");
        std::fs::write(theme_dir.join("style.css"), "body{}").expect("css");
        let theme = SiteTheme {
            name: "probe".to_string(),
            stylesheet: "body{}".to_string(),
            icons: std::collections::BTreeMap::new(),
            templates: std::collections::BTreeMap::from([(
                "page.html".to_string(),
                "{{ kind }}|{{ path }}|{% if note %}{{ note.title }}/{{ note.frontmatter.status }}{% else %}-{% endif %}|{{ vars.owner }}|{{ nav | length }}"
                    .to_string(),
            )]),
        };
        let mut notes = notes(vault.path());
        notes.push(note(
            vault.path(),
            E,
            "Landing",
            "tags: [programming]\nsite:\n  index: true\n",
            "Welcome.\n",
        ));
        let vault_ids = ids(&notes);
        let options = SiteOptions {
            index: Some(A.to_string()),
            vars: toml::from_str("owner = \"Acme\"\n").expect("vars"),
            ..SiteOptions::default()
        };
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[ViewDef::parse("by-status", "status").expect("valid view name")],
            options: &options,
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: Some((&theme, &theme_dir)),
            output_dir: None,
        })
        .expect("builds");
        let probe = |path: &str| text(&built, path);
        assert_eq!(
            probe("index.html"),
            "front|index.html|Rust Tips/done|Acme|2",
            "the index note's copy at the root is the front page"
        );
        assert_eq!(
            probe("notes/rust-tips.html"),
            "note|notes/rust-tips.html|Rust Tips/done|Acme|2"
        );
        assert_eq!(
            probe("tags/programming/index.html"),
            "group|tags/programming/index.html|Landing/|Acme|2",
            "a landing note renders its group's page"
        );
        assert_eq!(probe("tags/index.html"), "group|tags/index.html|-|Acme|2");
        assert_eq!(
            probe("views/by-status/done/index.html"),
            "group|views/by-status/done/index.html|-|Acme|2"
        );
    }

    #[test]
    fn a_note_names_its_template_and_a_missing_one_warns() {
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        const F: &str = "01FRZ3NDEKTSV4RRFFQ69G5FAV";
        let vault = vault();
        let theme_dir = vault.path().join(".ntropy/themes/site/probe");
        std::fs::create_dir_all(&theme_dir).expect("theme dir");
        std::fs::write(theme_dir.join("style.css"), "body{}").expect("css");
        let theme = SiteTheme {
            name: "probe".to_string(),
            stylesheet: "body{}".to_string(),
            icons: std::collections::BTreeMap::new(),
            templates: std::collections::BTreeMap::from([
                ("page.html".to_string(), "page:{{ title }}".to_string()),
                (
                    "splash.html".to_string(),
                    "splash:{{ title }}:{{ kind }}".to_string(),
                ),
            ]),
        };
        let mut notes = notes(vault.path());
        notes.push(note(
            vault.path(),
            E,
            "Welcome",
            "site:\n  template: splash\n",
            "Hello.\n",
        ));
        notes.push(note(
            vault.path(),
            F,
            "Legal",
            "site:\n  template: no-such-template\n",
            "Fine print.\n",
        ));
        let vault_ids = ids(&notes);
        let options = SiteOptions {
            index: Some(E.to_string()),
            ..SiteOptions::default()
        };
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &options,
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: Some((&theme, &theme_dir)),
            output_dir: None,
        })
        .expect("builds");
        assert_eq!(text(&built, "notes/welcome.html"), "splash:Welcome:note");
        assert_eq!(
            text(&built, "index.html"),
            "splash:Welcome:front",
            "the note's template renders its front-page copy too"
        );
        assert_eq!(text(&built, "notes/legal.html"), "page:Legal");
        assert_eq!(text(&built, "notes/plain.html"), "page:Plain");
        let template_warnings: Vec<&String> = built
            .warnings
            .iter()
            .filter(|warning| warning.contains("site.template"))
            .collect();
        assert_eq!(template_warnings.len(), 1, "{:?}", built.warnings);
        assert!(
            template_warnings[0].contains("legal.md")
                && template_warnings[0].contains("`no-such-template`")
                && template_warnings[0].contains("`page.html` is used"),
            "{}",
            template_warnings[0]
        );
    }

    #[test]
    fn the_builtin_theme_serves_no_templates() {
        let vault = vault();
        let notes = notes(vault.path());
        let vault_ids = ids(&notes);
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: None,
            output_dir: None,
        })
        .expect("builds");
        assert!(built.file("assets/style.css").is_some());
        assert!(
            built
                .files
                .iter()
                .all(|file| !file.path.starts_with("assets/templates/")),
            "no template is written under assets/"
        );
    }

    #[test]
    fn a_theme_template_that_fails_to_render_fails_the_build_naming_it() {
        let vault = vault();
        let theme_dir = vault.path().join(".ntropy/themes/site/corporate");
        std::fs::create_dir_all(&theme_dir).expect("theme dir");
        std::fs::write(theme_dir.join("style.css"), "body{}").expect("css");
        let theme = SiteTheme {
            name: "corporate".to_string(),
            stylesheet: "body{}".to_string(),
            icons: std::collections::BTreeMap::new(),
            templates: std::collections::BTreeMap::from([(
                "page.html".to_string(),
                "{{ title | no_such_filter }}".to_string(),
            )]),
        };
        let notes = notes(vault.path());
        let vault_ids = ids(&notes);
        let err = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: Some((&theme, &theme_dir)),
            output_dir: None,
        })
        .expect_err("the render error surfaces");
        let message = err.to_string();
        assert!(message.contains("page.html"), "{message}");
        assert!(message.contains("no_such_filter"), "{message}");
    }

    /// A documentation tree: a landing note with a label that asks for the
    /// group's listing, an ordered note, a hidden note, and a child group
    /// with a landing note that does not.
    fn docs_notes(vault: &Path) -> Vec<Note> {
        const E: &str = "01ERZ3NDEKTSV4RRFFQ69G5FAV";
        vec![
            note(
                vault,
                A,
                "Basics",
                "tags: [docs/start]\ndescription: Start here.\nsite:\n  index: true\n  label: Getting Started\n  order: 1\n  listing: true\n",
                "Welcome to the docs.\n\n```rust\nfn main() {}\n```\n",
            ),
            note(
                vault,
                B,
                "Install",
                "tags: [docs/start]\nsite:\n  order: 2\n",
                &format!("Read [the basics]({A}-basics.md) first.\n"),
            ),
            note(
                vault,
                C,
                "Secret",
                "tags: [docs/start]\nsite:\n  hidden: true\n  order: 3\n",
                "Not listed.\n",
            ),
            note(
                vault,
                E,
                "Gadgets",
                "tags: [docs/start/gadgets]\nsite:\n  index: true\n  order: 9\n",
                "About gadgets.\n",
            ),
        ]
    }

    fn build_docs(vault: &Path) -> Built {
        let notes = docs_notes(vault);
        let vault_ids = ids(&notes);
        build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Docs",
            vault_root: vault,
            theme: None,
            output_dir: None,
        })
        .expect("the site builds")
    }

    #[test]
    fn a_landing_note_is_rendered_as_its_group_page() {
        let vault = vault();
        let built = build_docs(vault.path());
        assert!(built.warnings.is_empty(), "{}", built.warnings.join("\n"));
        // No page of its own; the group page carries the note and then the
        // listing, without related notes.
        assert!(built.file("notes/basics.html").is_none());
        let start = text(&built, "tags/docs/start/index.html");
        assert!(
            start.contains("<h1 class=\"note-title\">Basics</h1>"),
            "{start}"
        );
        assert!(start.contains("Welcome to the docs."), "{start}");
        assert!(start.contains("<dt>description</dt>"), "{start}");
        assert!(!start.contains("<dt>site</dt>"), "{start}");
        assert!(
            start.contains("<a class=\"chip\" href=\"../../../tags/docs/start/gadgets/index.html\">gadgets<span class=\"count\">0</span></a>"),
            "{start}"
        );
        assert!(
            start.contains(
                "<a class=\"note-row-title\" href=\"../../../notes/install.html\">Install</a>"
            ),
            "{start}"
        );
        assert!(!start.contains("notes/secret.html"), "{start}");
        assert!(!start.contains("class=\"related\""), "{start}");
        // The breadcrumb stops above the group; the pager continues into
        // the group's entries.
        assert!(
            start.contains("<a href=\"../../../tags/index.html\">tags</a></li>\n<li><svg class=\"icon sep\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../../../tags/docs/index.html\">docs</a></li>\n</ol>"),
            "{start}"
        );
        assert!(
            start.contains("<a class=\"next\" rel=\"next\" href=\"../../../notes/install.html\">"),
            "{start}"
        );
        // Its grammar is loaded by the group page.
        assert!(start.contains("grammars/rust.js"), "{start}");
        // A link to the landing note reaches the group page, and the pager
        // calls it by its label.
        let install = text(&built, "notes/install.html");
        assert!(
            install.contains(
                "<a class=\"note-link\" href=\"../tags/docs/start/index.html\">Basics</a>"
            ),
            "{install}"
        );
        assert!(
            install.contains("<a class=\"prev\" rel=\"prev\" href=\"../tags/docs/start/index.html\"><svg class=\"icon\" aria-hidden=\"true\"><use href=\"#icon-chevron-left\"/></svg><span class=\"pager-text\"><span class=\"pager-label\">Previous</span><span class=\"pager-title\">Getting Started</span>"),
            "{install}"
        );
        // The sidebar names the group by the label; the child group's
        // landing page follows Install in the pager.
        assert!(install.contains(">Getting Started</a>"), "{install}");
        assert!(
            install.contains(
                "<a class=\"next\" rel=\"next\" href=\"../tags/docs/start/gadgets/index.html\">"
            ),
            "{install}"
        );
        // A landing note without `listing` stands alone on its page.
        let gadgets = text(&built, "tags/docs/start/gadgets/index.html");
        assert!(gadgets.contains("About gadgets."), "{gadgets}");
        assert!(!gadgets.contains("group-chips"), "{gadgets}");
        assert!(!gadgets.contains("note-rows"), "{gadgets}");
        assert!(!gadgets.contains("class=\"empty\""), "{gadgets}");
        // The search data sends the landing note to the group page.
        let data = text(&built, "assets/search-data.js");
        assert!(
            data.contains(&format!(
                "\"id\":\"{A}\",\"page\":\"tags/docs/start/index.html\""
            )),
            "{data}"
        );
    }

    #[test]
    fn a_root_starts_every_breadcrumb_there_and_a_query_can_be_the_root() {
        let vault = vault();
        let notes = docs_notes(vault.path());
        let vault_ids = ids(&notes);
        let options = SiteOptions {
            root: Some("tags/docs".to_string()),
            ..SiteOptions::default()
        };
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &options,
            query: None,
            fallback_title: "Docs",
            vault_root: vault.path(),
            theme: None,
            output_dir: None,
        })
        .expect("the site builds");
        // The root's child groups are the sidebar's sections, so a note
        // page starts its breadcrumb at its section, a section's landing
        // page has none, and a group below the section starts there.
        let install = text(&built, "notes/install.html");
        assert!(
            install.contains("<nav class=\"breadcrumbs\" aria-label=\"You are here\"><ol>\n<li><a href=\"../tags/docs/start/index.html\">Getting Started</a></li>\n</ol></nav>"),
            "{install}"
        );
        let start = text(&built, "tags/docs/start/index.html");
        assert!(!start.contains("class=\"breadcrumbs\""), "{start}");
        let gadgets = text(&built, "tags/docs/start/gadgets/index.html");
        assert!(
            gadgets.contains("<ol>\n<li><a href=\"../../../../tags/docs/start/index.html\">Getting Started</a></li>\n</ol>"),
            "{gadgets}"
        );
        // The root's own page is outside the sidebar and keeps the tag
        // tree's chain above it.
        let docs = text(&built, "tags/docs/index.html");
        assert!(
            docs.contains("<ol>\n<li><a href=\"../../tags/index.html\">tags</a></li>\n</ol>"),
            "{docs}"
        );
        assert!(
            install.contains("<summary class=\"nav-title\"><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/start/index.html\">Getting Started</a></summary>"),
            "{install}"
        );
        assert!(
            !install.contains(">docs<"),
            "the root wraps nothing: {install}"
        );

        // The export query stands in for the root.
        let query = crate::query::parse("tag:docs/start").expect("parses");
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: Some(&query),
            fallback_title: "Docs",
            vault_root: vault.path(),
            theme: None,
            output_dir: None,
        })
        .expect("the site builds");
        // The rooted group holds a note of its own, so it is the first
        // section, its child group the second.
        let install = text(&built, "notes/install.html");
        assert!(
            install.contains("<summary class=\"nav-title\"><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/start/index.html\">Getting Started</a></summary>"),
            "{install}"
        );
        assert!(
            install.contains("<summary class=\"nav-title\"><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/start/gadgets/index.html\">gadgets</a></summary>"),
            "{install}"
        );
        assert!(
            install.contains("<ol>\n<li><a href=\"../tags/docs/start/index.html\">Getting Started</a></li>\n</ol>"),
            "{install}"
        );
    }

    #[test]
    fn a_nav_table_gives_hand_made_crumbs_without_links() {
        let vault = vault();
        let notes = docs_notes(vault.path());
        let vault_ids = ids(&notes);
        let options: SiteOptions = toml::from_str(&format!(
            "[[nav]]\nlabel = \"Guide\"\nitems = [{{ label = \"Setup\", items = [{{ note = \"{B}\" }}] }}]\n"
        ))
        .expect("nav parses");
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &options,
            query: None,
            fallback_title: "Docs",
            vault_root: vault.path(),
            theme: None,
            output_dir: None,
        })
        .expect("the site builds");
        let install = text(&built, "notes/install.html");
        assert!(
            install.contains("<ol>\n<li><span>Guide</span></li>\n<li><svg class=\"icon sep\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><span>Setup</span></li>\n</ol>"),
            "{install}"
        );
        assert!(!install.contains("class=\"pager\""), "{install}");
        // A group the table never reaches keeps its section's breadcrumb.
        let start = text(&built, "tags/docs/start/index.html");
        assert!(
            start.contains("<li><a href=\"../../../tags/index.html\">tags</a></li>"),
            "{start}"
        );
    }

    #[test]
    fn related_notes_can_be_switched_off_for_the_site_or_for_one_note() {
        let vault = vault();
        let mut notes = docs_notes(vault.path());
        notes.push(note(
            vault.path(),
            "01FRZ3NDEKTSV4RRFFQ69G5FAV",
            "Insists",
            "tags: [docs/start]\nsite:\n  related: true\n",
            "Wants its related notes.\n",
        ));
        let vault_ids = ids(&notes);
        let build_with = |options: &SiteOptions| {
            build(&Input {
                notes: &notes,
                vault_ids: &vault_ids,
                views: &[],
                options,
                query: None,
                fallback_title: "Docs",
                vault_root: vault.path(),
                theme: None,
                output_dir: None,
            })
            .expect("the site builds")
        };
        let on = build_with(&SiteOptions::default());
        assert!(text(&on, "notes/install.html").contains("class=\"related\""));
        let off = build_with(&SiteOptions {
            related: Some(false),
            ..SiteOptions::default()
        });
        assert!(!text(&off, "notes/install.html").contains("class=\"related\""));
        assert!(text(&off, "notes/insists.html").contains("class=\"related\""));
    }

    #[test]
    fn a_hidden_note_has_a_page_but_no_place() {
        let vault = vault();
        let built = build_docs(vault.path());
        let secret = text(&built, "notes/secret.html");
        assert!(
            secret.contains("<h1 class=\"note-title\">Secret</h1>"),
            "{secret}"
        );
        assert!(!secret.contains("class=\"breadcrumbs\""), "{secret}");
        assert!(!secret.contains("class=\"pager\""), "{secret}");
        assert!(
            !secret.contains("notes/secret.html\""),
            "the sidebar does not list it: {secret}"
        );
        let data = text(&built, "assets/search-data.js");
        assert!(data.contains("\"title\":\"Secret\""), "{data}");
    }

    #[test]
    fn write_puts_every_file_and_copy_on_disk() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let out = tempfile::tempdir().expect("out dir");
        write(&built, out.path()).expect("writes");
        assert!(out.path().join("index.html").is_file());
        assert!(out.path().join("notes/rust-tips.html").is_file());
        assert!(
            out.path()
                .join("tags/programming/rust/index.html")
                .is_file()
        );
        assert!(out.path().join("assets/style.css").is_file());
        assert_eq!(
            std::fs::read(out.path().join("files/all-notes/diagram.png")).expect("copied"),
            b"png"
        );
        assert_eq!(
            std::fs::read(out.path().join("files/assets/logo.svg")).expect("copied"),
            b"svg"
        );
    }

    /// A site holding `files` and `copies` and nothing else.
    fn handmade(files: &[&str], copies: Vec<Copy>) -> Built {
        Built {
            files: files
                .iter()
                .map(|path| OutputFile {
                    path: path.to_string(),
                    contents: b"page".to_vec(),
                })
                .collect(),
            copies,
            warnings: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Write `built` into `<tmp>/out` and return the refusal it must end in.
    fn refused(tmp: &Path, built: &Built) -> std::io::Error {
        let err = write(built, &tmp.join("out")).expect_err("the target is refused");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "{err}");
        err
    }

    #[test]
    fn write_refuses_a_page_outside_the_output_directory() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let err = refused(tmp.path(), &handmade(&["../escape.html"], Vec::new()));
        assert!(err.to_string().contains("`../escape.html`"), "{err}");
        assert!(!tmp.path().join("escape.html").exists());
    }

    #[test]
    fn write_refuses_a_copy_outside_the_output_directory() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, b"source").expect("source");
        let copy = Copy {
            from: source,
            to: "files/../../escape.txt".to_string(),
        };
        refused(tmp.path(), &handmade(&[], vec![copy]));
        assert!(!tmp.path().join("escape.txt").exists());
    }

    #[test]
    fn write_refuses_an_absolute_target() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let elsewhere = tmp.path().join("elsewhere/escape.html");
        refused(
            tmp.path(),
            &handmade(&[&elsewhere.to_string_lossy()], Vec::new()),
        );
        assert!(!elsewhere.exists());
    }

    #[test]
    fn write_writes_no_file_when_a_later_target_is_refused() {
        let tmp = tempfile::tempdir().expect("temp dir");
        refused(
            tmp.path(),
            &handmade(&["index.html", "../escape.html"], Vec::new()),
        );
        assert!(!tmp.path().join("out/index.html").exists());
    }

    // =========================================================
    // What a link never brings into the site (ADR 0073)
    // =========================================================

    /// A vault holding, beside an ordinary asset, what an export must never
    /// copy: the vault's `.ntropy`, a repository at the root and two inside
    /// an asset folder (one a worktree's `.git` file), Finder metadata, a
    /// hidden directory, a folder holding only a hidden file, and a symlink
    /// into `.ntropy`.
    fn reserved_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        write_files(
            dir.path(),
            &[
                (".ntropy/config.toml", "[site]\n"),
                (".git/HEAD", "ref: refs/heads/main\n"),
                (".well-known/x", "x"),
                ("assets/pic.png", "png"),
                ("assets/.DS_Store", "ds"),
                ("assets/repo/.git/config", "[core]\n"),
                ("assets/wt/.git", "gitdir: /no-such-repository\n"),
                ("keep/.gitkeep", ""),
            ],
        );
        std::os::unix::fs::symlink(".ntropy", dir.path().join("cfg")).expect("symlink");
        dir
    }

    /// The site of one exported note whose body is `body`.
    fn site_linking(vault: &Path, body: &str) -> Built {
        let notes = vec![note(vault, A, "Linker", "", body)];
        let vault_ids = ids(&notes);
        build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault,
            theme: None,
            output_dir: None,
        })
        .expect("the site builds")
    }

    /// The copy targets of `built`, sorted, each with empty components
    /// dropped, so a doubled slash does not hide which file is copied.
    fn targets(built: &Built) -> Vec<String> {
        let mut targets: Vec<String> = built
            .copies
            .iter()
            .map(|copy| {
                copy.to
                    .split('/')
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        targets.sort();
        targets
    }

    /// `files` written into the vault, each with its parent directories.
    fn write_files(vault: &Path, files: &[(&str, &str)]) {
        for (path, contents) in files {
            let file = vault.join(path);
            std::fs::create_dir_all(file.parent().expect("a fixture file has a parent"))
                .expect("fixture directory");
            std::fs::write(&file, contents).expect("fixture file");
        }
    }

    #[test]
    fn a_linked_vault_root_leaves_listed_and_hidden_entries_out() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[root](/)\n");
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
    }

    #[test]
    fn a_link_into_ntropy_is_not_copied_and_stays_as_written() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[config](/.ntropy/config.toml)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
        let page = text(&built, "notes/linker.html");
        assert!(
            page.contains("<a href=\"/.ntropy/config.toml\">config</a>"),
            "{page}"
        );
        assert_eq!(
            built.warnings,
            [
                "referenced path `/.ntropy/config.toml` is or lies inside `.ntropy`, which exports never copy; not copied"
            ]
        );
    }

    #[test]
    fn a_link_to_the_ntropy_directory_is_not_copied() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[vault config](/.ntropy)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
    }

    #[test]
    fn a_link_into_git_is_not_copied_and_stays_as_written() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[head](/.git/HEAD)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
        let page = text(&built, "notes/linker.html");
        assert!(page.contains("<a href=\"/.git/HEAD\">head</a>"), "{page}");
    }

    #[test]
    fn a_repository_inside_an_asset_folder_is_left_out() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert!(
            !targets(&built).iter().any(|to| to.contains("/.git")),
            "{:?}",
            targets(&built)
        );
    }

    #[test]
    fn a_finder_metadata_file_in_an_asset_folder_is_left_out() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert!(
            !targets(&built).contains(&"files/assets/.DS_Store".to_string()),
            "{:?}",
            targets(&built)
        );
    }

    #[test]
    fn a_symlink_to_ntropy_is_refused() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[file](/cfg/config.toml) [directory](/cfg)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
    }

    #[test]
    fn a_case_variant_of_ntropy_is_refused() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[config](/.NTROPY/config.toml)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
        assert_eq!(
            built.warnings,
            [
                "referenced path `/.NTROPY/config.toml` is or lies inside `.NTROPY`, which exports never copy; not copied"
            ]
        );
    }

    // A case-insensitive filesystem holds `CVS` and `CVs` as one directory,
    // so each case gets a vault of its own.

    #[test]
    fn a_cvs_folder_is_left_out() {
        let vault = tempfile::tempdir().expect("temp dir");
        write_files(
            vault.path(),
            &[
                ("assets/pic.png", "png"),
                ("assets/CVS/Entries", "/x/1.1///\n"),
            ],
        );
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
    }

    #[test]
    fn a_folder_named_cvs_in_another_case_is_copied() {
        let vault = tempfile::tempdir().expect("temp dir");
        write_files(vault.path(), &[("assets/CVs/resume.pdf", "pdf")]);
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert_eq!(targets(&built), ["files/assets/CVs/resume.pdf"]);
    }

    #[test]
    fn a_direct_link_to_a_hidden_file_is_copied() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[well known](/.well-known/x)\n");
        assert_eq!(targets(&built), ["files/.well-known/x"]);
    }

    #[test]
    fn a_hidden_entry_below_a_linked_directory_is_left_out() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[root](/)\n");
        assert!(
            !targets(&built).contains(&"files/.well-known/x".to_string()),
            "{:?}",
            targets(&built)
        );
    }

    #[test]
    fn a_linked_hidden_directory_still_leaves_its_hidden_entries_out() {
        let vault = reserved_vault();
        std::fs::write(vault.path().join(".well-known/.secret"), "s").expect("hidden file");
        let built = site_linking(vault.path(), "[well known](/.well-known)\n");
        assert_eq!(targets(&built), ["files/.well-known/x"]);
    }

    #[test]
    fn a_folder_of_only_left_out_entries_has_nothing_to_copy() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[keep](/keep)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
        assert_eq!(
            built.warnings,
            ["referenced directory `/keep` has nothing to copy"]
        );
    }

    #[test]
    fn a_linked_vault_root_warns_about_ntropy_by_its_paths() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[root](/)\n");
        assert_eq!(
            built.warnings,
            ["referenced directory `/`: left out `/.ntropy`, `/cfg`"]
        );
    }

    #[test]
    fn a_linked_vault_root_notes_the_routine_metadata_it_left_out() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[root](/)\n");
        assert_eq!(
            built.notes,
            [
                "referenced directory `/`: left out 2 hidden entries and 4 never-copied entries (`.DS_Store`, `.git`)"
            ]
        );
    }

    #[test]
    fn a_linked_vault_root_copies_without_an_empty_path_component() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[root](/)\n");
        assert!(
            built
                .copies
                .iter()
                .all(|copy| copy.to.split('/').all(|part| !part.is_empty())),
            "{:?}",
            targets(&built)
        );
    }

    #[test]
    fn overlapping_directory_links_copy_each_file_once() {
        let vault = reserved_vault();
        let built = site_linking(vault.path(), "[root](/) [assets](/assets)\n");
        let all = targets(&built);
        let mut unique = all.clone();
        unique.dedup();
        assert_eq!(all, unique);
    }

    #[test]
    fn a_vault_theme_leaves_its_repository_and_hidden_files_out() {
        let vault = reserved_vault();
        let theme_dir = vault.path().join(".ntropy/themes/site/corporate");
        write_files(
            &theme_dir,
            &[
                ("style.css", "body {}"),
                ("fonts/a.woff2", "font"),
                (".git/HEAD", "ref: refs/heads/main\n"),
                (".DS_Store", "ds"),
                (".editorconfig", "root = true\n"),
            ],
        );
        let notes = vec![note(vault.path(), A, "Linker", "", "text\n")];
        let vault_ids = ids(&notes);
        let theme = SiteTheme {
            name: "corporate".to_string(),
            ..SiteTheme::builtin()
        };
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: Some((&theme, &theme_dir)),
            output_dir: None,
        })
        .expect("the site builds");
        assert_eq!(
            targets(&built),
            ["assets/fonts/a.woff2", "assets/style.css"]
        );
        assert_eq!(
            built.notes,
            [
                "theme `corporate`: left out 1 hidden entry and 2 never-copied entries (`.DS_Store`, `.git`)"
            ]
        );
    }

    #[test]
    fn a_link_to_a_directory_holding_the_output_does_not_mirror_the_site() {
        let vault = reserved_vault();
        let output = vault.path().join("site");
        write_files(vault.path(), &[("site/index.html", "earlier run")]);
        let notes = vec![note(vault.path(), A, "Linker", "", "[root](/)\n")];
        let vault_ids = ids(&notes);
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: None,
            output_dir: Some(&output),
        })
        .expect("the site builds");
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
    }

    // =========================================================
    // Symlinks that lead out of the vault (ADR 0073)
    // =========================================================

    /// A directory beside the vault, standing for anywhere else on the
    /// machine, holding `secret.txt` and `pictures/photo.jpg`.
    fn elsewhere() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        write_files(
            dir.path(),
            &[("secret.txt", "secret"), ("pictures/photo.jpg", "jpg")],
        );
        dir
    }

    fn symlink(target: &Path, link: &Path) {
        std::fs::create_dir_all(link.parent().expect("a link has a parent")).expect("link dir");
        std::os::unix::fs::symlink(target, link).expect("symlink");
    }

    #[test]
    fn a_link_to_a_file_symlinked_out_of_the_vault_is_not_copied() {
        let vault = reserved_vault();
        let outside = elsewhere();
        symlink(
            &outside.path().join("secret.txt"),
            &vault.path().join("assets/secret.txt"),
        );
        let built = site_linking(vault.path(), "[secret](/assets/secret.txt)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
        assert_eq!(
            built.warnings,
            ["referenced path `/assets/secret.txt` resolves outside the vault; not copied"]
        );
        let page = text(&built, "notes/linker.html");
        assert!(
            page.contains("<a href=\"/assets/secret.txt\">secret</a>"),
            "{page}"
        );
    }

    #[test]
    fn a_symlinked_asset_folder_out_of_the_vault_is_refused() {
        let vault = reserved_vault();
        let outside = elsewhere();
        symlink(&outside.path().join("pictures"), &vault.path().join("pics"));
        let built = site_linking(
            vault.path(),
            "[pictures](/pics) ![photo](/pics/photo.jpg)\n",
        );
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
    }

    #[test]
    fn a_file_symlinked_out_of_the_vault_is_left_out_of_a_linked_directory() {
        let vault = reserved_vault();
        let outside = elsewhere();
        symlink(
            &outside.path().join("secret.txt"),
            &vault.path().join("assets/secret.txt"),
        );
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
        assert_eq!(
            built.warnings,
            ["referenced directory `/assets`: left out 1 entry that resolves outside the vault"]
        );
    }

    #[test]
    fn a_symlinked_all_notes_leaves_the_images_beside_the_notes_out() {
        let vault = tempfile::tempdir().expect("temp dir");
        let outside = elsewhere();
        write_files(outside.path(), &[("notes/diagram.png", "png")]);
        symlink(
            &outside.path().join("notes"),
            &vault.path().join("all-notes"),
        );
        let built = site_linking(vault.path(), "![diagram](diagram.png)\n");
        assert!(targets(&built).is_empty(), "{:?}", targets(&built));
        assert!(
            built.file("notes/linker.html").is_some(),
            "the note's page is exported"
        );
    }

    #[test]
    fn a_dangling_symlink_in_a_linked_directory_is_reported() {
        let vault = reserved_vault();
        symlink(
            &vault.path().join("no-such-target"),
            &vault.path().join("assets/dangling"),
        );
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
        assert_eq!(
            built.warnings,
            ["referenced directory `/assets`: left out 1 entry that cannot be resolved"]
        );
    }

    #[test]
    fn a_link_to_a_dangling_symlink_is_reported() {
        let vault = reserved_vault();
        symlink(
            &vault.path().join("no-such-target"),
            &vault.path().join("assets/dangling"),
        );
        let built = site_linking(vault.path(), "[dangling](/assets/dangling)\n");
        assert_eq!(
            built.warnings,
            ["referenced path `/assets/dangling` cannot be resolved; not copied"]
        );
    }

    #[test]
    fn a_symlinked_directory_in_a_linked_directory_is_skipped() {
        let vault = reserved_vault();
        write_files(vault.path(), &[("gallery/a.jpg", "a")]);
        symlink(
            &vault.path().join("gallery"),
            &vault.path().join("assets/gallery"),
        );
        let built = site_linking(vault.path(), "[assets](/assets)\n");
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
        assert_eq!(
            built.warnings,
            ["referenced directory `/assets`: left out 1 symlinked directory"]
        );
    }

    #[test]
    fn a_symlink_that_stays_inside_the_vault_is_followed() {
        let vault = reserved_vault();
        symlink(
            &vault.path().join("assets/pic.png"),
            &vault.path().join("alias.png"),
        );
        let built = site_linking(vault.path(), "![alias](/alias.png)\n");
        assert_eq!(targets(&built), ["files/alias.png"]);
    }

    #[test]
    fn a_vault_given_through_a_symlink_is_judged_by_where_it_resolves() {
        let parent = tempfile::tempdir().expect("temp dir");
        let vault = reserved_vault();
        let outside = elsewhere();
        symlink(
            &outside.path().join("secret.txt"),
            &vault.path().join("assets/secret.txt"),
        );
        let through = parent.path().join("vault");
        symlink(vault.path(), &through);
        let built = site_linking(
            &through,
            "![pic](/assets/pic.png) [s](/assets/secret.txt)\n",
        );
        assert_eq!(targets(&built), ["files/assets/pic.png"]);
    }

    /// A vault theme `corporate` holding `style.css`, plus `files`.
    fn theme_in(vault: &Path, files: &[(&str, &str)]) -> PathBuf {
        let theme_dir = vault.join(".ntropy/themes/site/corporate");
        write_files(&theme_dir, &[("style.css", "body {}")]);
        write_files(&theme_dir, files);
        theme_dir
    }

    fn site_with_theme(vault: &Path, theme_dir: &Path) -> Built {
        let notes = vec![note(vault, A, "Linker", "", "text\n")];
        let vault_ids = ids(&notes);
        let theme = SiteTheme {
            name: "corporate".to_string(),
            ..SiteTheme::builtin()
        };
        build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
            query: None,
            fallback_title: "Vault",
            vault_root: vault,
            theme: Some((&theme, theme_dir)),
            output_dir: None,
        })
        .expect("the site builds")
    }

    #[test]
    fn a_theme_file_symlinked_into_ntropy_is_refused() {
        let vault = reserved_vault();
        let theme_dir = theme_in(vault.path(), &[]);
        symlink(
            Path::new("../../../../config.toml"),
            &theme_dir.join("fonts/x.woff2"),
        );
        let built = site_with_theme(vault.path(), &theme_dir);
        assert_eq!(targets(&built), ["assets/style.css"]);
    }

    #[test]
    fn a_theme_file_symlinked_out_of_the_vault_is_refused() {
        let vault = reserved_vault();
        let outside = elsewhere();
        let theme_dir = theme_in(vault.path(), &[]);
        symlink(
            &outside.path().join("secret.txt"),
            &theme_dir.join("fonts/x.woff2"),
        );
        let built = site_with_theme(vault.path(), &theme_dir);
        assert_eq!(targets(&built), ["assets/style.css"]);
        assert_eq!(
            built.warnings,
            ["theme `corporate`: left out 1 entry that resolves outside the theme and the vault"]
        );
    }

    #[test]
    fn a_theme_file_symlinked_elsewhere_in_the_vault_is_copied() {
        let vault = reserved_vault();
        let theme_dir = theme_in(vault.path(), &[]);
        symlink(
            &vault.path().join("assets/pic.png"),
            &theme_dir.join("pic.png"),
        );
        let built = site_with_theme(vault.path(), &theme_dir);
        assert_eq!(targets(&built), ["assets/pic.png", "assets/style.css"]);
    }
}
