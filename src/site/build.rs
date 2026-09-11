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
use crate::render::{LinkTarget, RenderError, ResolvedLink};
use crate::view::ViewDef;

use super::SiteOptions;
use super::frontend::{self, Grammars};
use super::model::{self, Front, Landing, Model, Placement};
use super::nav;
use super::page::{Crumb, NoteContext, NoteFragment, Page, PageKind, PageLink, TagLink, Templates};
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
    /// note that is not exported.
    pub warnings: Vec<String>,
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
    let mut files = Vec::new();
    let mut copies = Vec::new();
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
    // templates were rendered with and are not served.
    match input.theme {
        Some((_, dir)) => {
            for (relative, from) in files_under(dir)? {
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

    copies.extend(mirrored(input.vault_root, &referenced, &mut warnings)?);

    Ok(Built {
        files,
        copies,
        warnings,
    })
}

/// The copies that mirror `referenced`, root-absolute vault paths, under
/// `files/`. A linked directory is mirrored with its whole tree, so the
/// link resolves in the site the way it does in the vault; an empty
/// directory and a path that does not exist are warnings.
pub(crate) fn mirrored(
    vault_root: &Path,
    referenced: &[String],
    warnings: &mut Vec<String>,
) -> Result<Vec<Copy>, crate::fsutil::FsError> {
    let mut copies = Vec::new();
    for path in referenced {
        let from = vault_root.join(path.trim_start_matches('/'));
        if from.is_file() {
            copies.push(Copy {
                from,
                to: format!("{FILES_DIR}{path}"),
            });
        } else if from.is_dir() {
            let inside = files_under(&from)?;
            if inside.is_empty() {
                warnings.push(format!(
                    "referenced directory is empty, nothing to copy: {}",
                    from.display()
                ));
            }
            for (relative, file) in inside {
                copies.push(Copy {
                    from: file,
                    to: format!("{FILES_DIR}{path}/{relative}"),
                });
            }
        } else {
            warnings.push(format!(
                "referenced file not found in the vault: {}",
                from.display()
            ));
        }
    }
    Ok(copies)
}

/// Write a built site into `dir`, creating directories as needed.
pub fn write(built: &Built, dir: &Path) -> std::io::Result<()> {
    for file in &built.files {
        let path = dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &file.contents)?;
    }
    for copy in &built.copies {
        let path = dir.join(&copy.to);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&copy.from, path)?;
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
struct PageTargets<'a> {
    model: &'a Model,
    prefix: String,
    note_dir: &'a str,
}

impl Targets for PageTargets<'_> {
    fn note_href(&self, id: Id, slug: &str) -> String {
        match self.model.page_of(id) {
            Some(page) => format!("{}{page}", self.prefix),
            None => format!("{}notes/{slug}.html", self.prefix),
        }
    }

    fn asset_href(&self, dest: &str) -> String {
        match resolve_root_relative(self.note_dir, dest) {
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
/// and icon sprite, and the config's `[site.vars]`.
struct Export<'a> {
    model: &'a Model,
    templates: &'a Templates,
    icons: &'a str,
    vars: &'a toml::Table,
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
        let views = [ViewDef::new("by-status", "status")];
        build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &views,
            options,
            query: None,
            fallback_title: "Vault",
            vault_root: vault,
            theme: None,
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
            joined.contains("referenced directory is empty, nothing to copy")
                && joined.contains("assets/empty"),
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
            views: &[ViewDef::new("by-status", "status")],
            options: &options,
            query: None,
            fallback_title: "Vault",
            vault_root: vault.path(),
            theme: Some((&theme, &theme_dir)),
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
        })
        .expect("the site builds");
        // A note page, a landing page, and a plain group page below the
        // root all start at the root, which links to its page.
        let install = text(&built, "notes/install.html");
        assert!(
            install.contains("<nav class=\"breadcrumbs\" aria-label=\"You are here\"><ol>\n<li><a href=\"../tags/docs/index.html\">docs</a></li>\n<li><svg class=\"icon sep\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/start/index.html\">Getting Started</a></li>\n</ol></nav>"),
            "{install}"
        );
        let start = text(&built, "tags/docs/start/index.html");
        assert!(
            start.contains(
                "<ol>\n<li><a href=\"../../../tags/docs/index.html\">docs</a></li>\n</ol>"
            ),
            "{start}"
        );
        let gadgets = text(&built, "tags/docs/start/gadgets/index.html");
        assert!(
            gadgets.contains("<li><a href=\"../../../../tags/docs/index.html\">docs</a></li>\n<li><svg class=\"icon sep\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../../../../tags/docs/start/index.html\">Getting Started</a></li>"),
            "{gadgets}"
        );
        // The root's own page has no breadcrumb above it but the section.
        let docs = text(&built, "tags/docs/index.html");
        assert!(
            docs.contains("<ol>\n<li><a href=\"../../tags/index.html\">tags</a></li>\n</ol>"),
            "{docs}"
        );
        assert!(
            install.contains("<a class=\"nav-all\" href=\"../tags/docs/index.html\">Overview</a>"),
            "{install}"
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
        })
        .expect("the site builds");
        let install = text(&built, "notes/install.html");
        assert!(
            install.contains(
                "<a class=\"nav-all\" href=\"../tags/docs/start/index.html\">Overview</a>"
            ),
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
}
