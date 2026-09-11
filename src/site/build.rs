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
use crate::render::html::{self, Targets, frontmatter};
use crate::render::markdown::resolve_root_relative;
use crate::render::{LinkTarget, ResolvedLink};
use crate::view::ViewDef;

use super::SiteOptions;
use super::frontend::{self, Grammars};
use super::model::{Front, Model, Placement};
use super::nav;
use super::page::{NoteFragment, Page, PageLink};
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
    let model = Model::build(
        input.notes,
        input.views,
        input.options,
        input.fallback_title,
    )?;
    let mut warnings = model.warnings.clone();
    let mut files = Vec::new();
    let mut copies = Vec::new();

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

    for (index, note) in ordered.iter().enumerate() {
        let entry = &model.notes[index];
        let links = resolve_links(note, &by_id, input.vault_ids, &mut warnings);
        let note_dir = root_absolute_dir(input.vault_root, &note.path);

        let rendered = render_note(
            &model,
            index,
            note,
            &links,
            &note_dir,
            &entry.page,
            &grammars,
            &mut warnings,
        );
        files.push(OutputFile {
            path: entry.page.clone(),
            contents: rendered.page.into_bytes(),
        });
        used_grammars.extend(rendered.grammars);
        for dest in rendered.assets {
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
                &model,
                index,
                note,
                &links,
                &note_dir,
                INDEX_PAGE,
                &grammars,
                &mut repeated,
            );
            files.push(OutputFile {
                path: INDEX_PAGE.to_string(),
                contents: rendered.page.into_bytes(),
            });
        }
    }

    files.push(OutputFile {
        path: format!("{ASSETS_DIR}/{}", frontend::APP_SCRIPT),
        contents: frontend::APP_JS.as_bytes().to_vec(),
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
            &model,
            INDEX_PAGE,
            &model.title,
            body,
            Vec::new(),
            String::new(),
            None,
            None,
            &[],
        );
        files.push(OutputFile {
            path: INDEX_PAGE.to_string(),
            contents: page.render().into_bytes(),
        });
    }

    // Section indexes and group pages.
    for section in &model.sections {
        let prefix = nav::prefix_for(&section.page);
        let body = format!(
            "<h1>{}</h1>\n{}",
            html::writer::escape(&section.title),
            nav::group_list(&model, &section.groups, &prefix)
        );
        let page = chrome(
            &model,
            &section.page,
            &section.title,
            body,
            Vec::new(),
            String::new(),
            None,
            None,
            &[],
        );
        files.push(OutputFile {
            path: section.page.clone(),
            contents: page.render().into_bytes(),
        });

        let mut stack: Vec<(&super::model::Group, Vec<PageLink>)> = section
            .groups
            .iter()
            .map(|group| (group, Vec::new()))
            .collect();
        while let Some((group, parents)) = stack.pop() {
            let prefix = nav::prefix_for(&group.page);
            let mut crumbs = vec![PageLink {
                label: section.title.clone(),
                href: format!("{prefix}{}", section.page),
            }];
            for parent in &parents {
                crumbs.push(PageLink {
                    label: parent.label.clone(),
                    href: format!("{prefix}{}", parent.href),
                });
            }
            let body = format!(
                "<h1>{}</h1>\n{}{}",
                html::writer::escape(&group.value),
                nav::group_list(&model, &group.children, &prefix),
                nav::listing(&model, &group.descendants(&model), &prefix)
            );
            let page = chrome(
                &model,
                &group.page,
                &group.value,
                body,
                crumbs,
                String::new(),
                None,
                None,
                &[],
            );
            files.push(OutputFile {
                path: group.page.clone(),
                contents: page.render().into_bytes(),
            });
            let mut trail = parents.clone();
            trail.push(PageLink {
                label: group.label.clone(),
                href: group.page.clone(),
            });
            for child in &group.children {
                stack.push((child, trail.clone()));
            }
        }
    }

    // The theme's files under `assets/`, the stylesheet among them.
    match input.theme {
        Some((_, dir)) => {
            for (relative, from) in files_under(dir)? {
                copies.push(Copy {
                    from,
                    to: format!("{ASSETS_DIR}/{relative}"),
                });
            }
        }
        None => {
            for (relative, contents) in theme::BUILTIN_FILES {
                files.push(OutputFile {
                    path: format!("{ASSETS_DIR}/{relative}"),
                    contents: contents.to_vec(),
                });
            }
        }
    }

    // Referenced vault paths, mirrored under `files/`. A linked directory
    // is mirrored with its whole tree, so the link resolves in the site the
    // way it does in the vault.
    for path in referenced {
        let from = input.vault_root.join(path.trim_start_matches('/'));
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

    Ok(Built {
        files,
        copies,
        warnings,
    })
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
fn root_absolute_dir(vault_root: &Path, note_path: &Path) -> String {
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

/// Render one note as a site page at `at` (its own page, or `index.html`
/// when it is the front page). A fence language without a grammar is a
/// warning; its block stays plain.
///
/// Every argument is a distinct input of the page; bundling them into a
/// struct would name the same eight things one level down.
#[allow(clippy::too_many_arguments)]
fn render_note(
    model: &Model,
    index: usize,
    note: &Note,
    links: &[ResolvedLink],
    note_dir: &str,
    at: &str,
    grammars: &Grammars,
    warnings: &mut Vec<String>,
) -> RenderedNote {
    let prefix = nav::prefix_for(at);
    let targets = PageTargets {
        model,
        prefix: prefix.clone(),
        note_dir,
    };
    let emitted = html::emit(&note.body, links, &targets);
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

    let body = NoteFragment {
        title: entry.title.clone(),
        created: entry.created.clone(),
        tags: entry.tags.clone(),
        fields: frontmatter::fields(&note.frontmatter),
        body: emitted.html,
    }
    .render();

    let placement = model.placements[index].as_ref();
    let crumbs = placement
        .map(|placement| breadcrumbs(model, placement, &prefix))
        .unwrap_or_default();
    let neighbour = |neighbour: Option<usize>| {
        neighbour.map(|other| PageLink {
            label: model.notes[other].title.clone(),
            href: format!("{prefix}{}", model.notes[other].page),
        })
    };
    let prev = placement.and_then(|placement| neighbour(placement.prev));
    let next = placement.and_then(|placement| neighbour(placement.next));

    let page = chrome(
        model,
        at,
        &entry.title,
        body,
        crumbs,
        nav::outline(&emitted.headings),
        prev,
        next,
        &needed,
    );
    RenderedNote {
        page: page.render(),
        assets: emitted.assets,
        grammars: needed,
    }
}

/// The breadcrumb of a placed note: the section, then each group down to
/// the note's own.
fn breadcrumbs(model: &Model, placement: &Placement, prefix: &str) -> Vec<PageLink> {
    let section = &model.sections[placement.section];
    let mut crumbs = vec![PageLink {
        label: section.title.clone(),
        href: format!("{prefix}{}", section.page),
    }];
    for group in model.trail(placement) {
        crumbs.push(PageLink {
            label: group.label.clone(),
            href: format!("{prefix}{}", group.page),
        });
    }
    crumbs
}

/// The chrome of a page at `at`: sidebar with the current trail open, the
/// stylesheet and scripts relative to the page (the page script always, one
/// script per grammar in `grammars`), and the given navigation.
#[allow(clippy::too_many_arguments)]
fn chrome(
    model: &Model,
    at: &str,
    title: &str,
    body: String,
    breadcrumbs: Vec<PageLink>,
    outline: String,
    prev: Option<PageLink>,
    next: Option<PageLink>,
    grammars: &[String],
) -> Page {
    let prefix = nav::prefix_for(at);
    let mut scripts = vec![format!("{prefix}{ASSETS_DIR}/{}", frontend::APP_SCRIPT)];
    for grammar in grammars {
        scripts.push(format!(
            "{prefix}{ASSETS_DIR}/{}/{grammar}.js",
            frontend::GRAMMARS_DIR
        ));
    }
    Page {
        lang: model.lang.clone(),
        site_title: model.title.clone(),
        title: title.to_string(),
        stylesheet: format!("{prefix}{ASSETS_DIR}/{}", theme::STYLESHEET),
        scripts,
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
fn files_under(dir: &Path) -> Result<Vec<(String, PathBuf)>, crate::fsutil::FsError> {
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
            page.contains("<li><a href=\"../views/by-status/done/index.html\">done</a></li>"),
            "{page}"
        );
        assert!(
            page.contains(
                "<details open><summary><a href=\"../views/by-status/done/index.html\">done</a>"
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
        assert!(!page.contains("grammars/cobol.js"), "{page}");
        // A page without code loads the page script alone.
        let plain = text(&built, "notes/plain.html");
        assert!(plain.contains("assets/app.js"), "{plain}");
        assert!(!plain.contains("assets/grammars/"), "{plain}");
        // The used grammar is written once, as a registering script; the
        // page script is the embedded one.
        let script = text(&built, "assets/grammars/rust.js");
        assert!(script.contains("w.__ntropyGrammars.push({"), "{script}");
        assert!(script.contains("\"name\":\"rust\""), "{script}");
        assert!(built.file("assets/grammars/cobol.js").is_none());
        assert_eq!(text(&built, "assets/app.js"), frontend::APP_JS);
    }

    #[test]
    fn every_page_mounts_the_search_over_the_embedded_data() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        let page = text(&built, "notes/rust-tips.html");
        assert!(
            page.contains(
                "<div data-search=\"../assets/search-data.js\" data-prefix=\"../\"></div>"
            ),
            "{page}"
        );
        let index = text(&built, "index.html");
        assert!(
            index.contains("<div data-search=\"assets/search-data.js\" data-prefix=\"\"></div>"),
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
    }

    #[test]
    fn neighbours_follow_the_first_group() {
        let vault = vault();
        let built = build_site(vault.path(), &SiteOptions::default());
        // `Plain` is alone in `open`; `Rust Tips` alone in `done`: no pager.
        let page = text(&built, "notes/plain.html");
        assert!(!page.contains("class=\"pager\""), "{page}");
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
                "<a href=\"../tags/programming/index.html\">programming</a> <span class=\"count\">1</span>"
            ),
            "{tags}"
        );
        let programming = text(&built, "tags/programming/index.html");
        assert!(
            programming.contains("<h1>programming</h1>"),
            "{programming}"
        );
        assert!(
            programming.contains("<a href=\"../../tags/programming/rust/index.html\">rust</a>"),
            "{programming}"
        );
        assert!(
            programming.contains("<a href=\"../../notes/rust-tips.html\">Rust Tips</a>"),
            "{programming}"
        );
        let rust = text(&built, "tags/programming/rust/index.html");
        assert!(
            rust.contains("<li><a href=\"../../../tags/index.html\">tags</a></li>"),
            "{rust}"
        );
        assert!(
            rust.contains(
                "<li><a href=\"../../../tags/programming/index.html\">programming</a></li>"
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
        let theme = SiteTheme {
            name: "corporate".to_string(),
            stylesheet: "body{}".to_string(),
        };
        let notes = notes(vault.path());
        let vault_ids = ids(&notes);
        let built = build(&Input {
            notes: &notes,
            vault_ids: &vault_ids,
            views: &[],
            options: &SiteOptions::default(),
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
        assert_eq!(theme_copies, ["assets/fonts/a.woff2", "assets/style.css"]);
        assert!(
            built.file("assets/style.css").is_none(),
            "the built-in stylesheet is not written"
        );
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
