//! The engine behind the `html` format of `render` (ADR 0046, ADR 0057).
//!
//! The site's page for one note, without the parts that need a site:
//! `<stem>.html` and, beside it, `<stem>_files/` holding the theme's files,
//! the page script, the grammars the page's code blocks need, and the
//! files the note references, laid out as the site's `assets/` is so the
//! theme's stylesheet works unchanged. Note links target `<slug>.html`
//! beside the artifact, the HTML counterpart of the `<slug>.pdf` convention
//! (ADR 0044). The site export builds its navigation around the same
//! converted body.

use super::emitter::{self, Targets};
use super::frontmatter;
use crate::id::Id;
use crate::render::markdown::resolve_root_relative;
use crate::render::{PreparedDocument, RenderContext, RenderError, Renderer, files_dir};
use crate::site::build::{self, FILES_DIR};
use crate::site::page::{NoteFragment, Page, TagLink, Templates};
use crate::site::{DocumentSettings, SiteTheme, frontend, nav, theme};

/// The theme directory whose files stay out of the artifact: the icons are
/// inlined as a sprite, since a `<use>` across files does not work over
/// `file://`.
const ICONS_PREFIX: &str = "icons/";

/// The ntropy-owned engine registered as `html`.
pub struct Html {
    settings: DocumentSettings,
}

impl Html {
    pub fn new(settings: DocumentSettings) -> Self {
        Html { settings }
    }
}

/// Where the page's links point: note links at the sibling artifact by
/// default name, vault files at their mirror inside the files directory,
/// both relative to the page; a path that climbs out of the vault stays
/// as written, unresolved.
struct DocumentTargets<'a> {
    /// The files directory's name, `<stem>_files`.
    files: &'a str,
    /// The note's directory as a root-absolute path within the vault.
    note_dir: &'a str,
}

impl Targets for DocumentTargets<'_> {
    fn note_href(&self, _id: Id, slug: &str) -> String {
        format!("{slug}.html")
    }

    fn asset_href(&self, dest: &str) -> String {
        match resolve_root_relative(self.note_dir, dest) {
            Some(path) => format!("{}/{FILES_DIR}{path}", self.files),
            None => dest.to_string(),
        }
    }
}

impl Renderer for Html {
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError> {
        let files_path = files_dir(ctx.output_path());
        let files = files_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let note_dir = build::root_absolute_dir(&doc.vault_root, &doc.path);
        let targets = DocumentTargets {
            files: &files,
            note_dir: &note_dir,
        };
        let emitted = emitter::emit(&doc.body, &doc.links, &targets, Some(&doc.title));
        for warning in &emitted.warnings {
            ctx.warn(&warning.message);
        }

        // The theme's files, the icons and the templates excepted: a vault
        // theme's from its directory, the built-in theme's from the binary.
        let theme = SiteTheme::selected(self.settings.theme.as_ref());
        let templates = Templates::new(&theme)?;
        match &self.settings.theme_dir {
            Some(dir) => {
                for (relative, from) in build::files_under(dir)? {
                    if !relative.starts_with(ICONS_PREFIX) && !theme::is_template(&relative) {
                        ctx.copy_file(&from, &relative)?;
                    }
                }
            }
            None => {
                for asset in theme::BUILTIN_FILES {
                    if !asset.path.starts_with(ICONS_PREFIX) && !theme::is_template(asset.path) {
                        ctx.write_file(asset.path, &asset.contents())?;
                    }
                }
            }
        }

        // The grammars the code blocks need, each grammar it embeds included,
        // written as the site writes them; a language without a grammar is
        // a warning and its block stays plain.
        let grammars = frontend::grammars()?;
        let mut wanted: Vec<&str> = Vec::new();
        for language in &emitted.languages {
            match grammars.resolve(language) {
                Some(grammar) => wanted.push(grammar.name.as_str()),
                None => ctx.warn(&format!(
                    "no highlighting grammar for `{language}`; the block stays plain"
                )),
            }
        }
        let needed = grammars.closure(wanted);
        let mut scripts = Vec::new();
        for grammar in &needed {
            let relative = format!("{}/{}", frontend::GRAMMARS_DIR, grammar.file_name());
            ctx.write_file(&relative, grammar.script().as_bytes())?;
            scripts.push(format!("{files}/{relative}"));
        }
        ctx.write_file(frontend::APP_SCRIPT, frontend::app_js().as_bytes())?;
        scripts.push(format!("{files}/{}", frontend::APP_SCRIPT));

        // The files the note references, mirrored as the site mirrors them.
        let mut referenced: Vec<String> = Vec::new();
        for dest in &emitted.assets {
            match resolve_root_relative(&note_dir, dest) {
                Some(path) => {
                    if !referenced.contains(&path) {
                        referenced.push(path);
                    }
                }
                None => ctx.warn(&format!(
                    "`{dest}` points outside the vault and is not copied"
                )),
            }
        }
        let mut warnings = Vec::new();
        for copy in build::mirrored(&doc.vault_root, &referenced, &mut warnings)? {
            ctx.copy_file(&copy.from, &copy.to)?;
        }
        for warning in &warnings {
            ctx.warn(warning);
        }

        let body = NoteFragment {
            title: doc.title.clone(),
            created: doc.created.clone(),
            tags: TagLink::plain(&doc.tags),
            fields: frontmatter::fields(&doc.frontmatter),
            body: emitted.html,
        }
        .render(&templates)?;
        let page = Page {
            lang: self.settings.lang.clone(),
            site_title: doc.title.clone(),
            title: doc.title.clone(),
            prefix: String::new(),
            stylesheet: format!("{files}/{}", theme::STYLESHEET),
            scripts,
            icons: theme.sprite(),
            sidebar: String::new(),
            breadcrumbs: Vec::new(),
            outline: nav::outline(&emitted.headings),
            prev: None,
            next: None,
            body,
            document: true,
        };
        ctx.write_output(page.render(&templates)?.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Invocation, LinkTarget, ResolvedLink, ToolOutput};

    use std::collections::BTreeMap;
    use std::ops::Range;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// A [`RenderContext`] that records the artifact, the files written and
    /// copied beside it, and the warnings. The html engine runs no tool and
    /// stages nothing; both panic if reached.
    struct FakeContext {
        output: PathBuf,
        warnings: Vec<String>,
        written: Option<Vec<u8>>,
        files: BTreeMap<String, Vec<u8>>,
        copies: Vec<(PathBuf, String)>,
    }

    impl FakeContext {
        fn new() -> Self {
            FakeContext {
                output: PathBuf::from("/artifacts/note.html"),
                warnings: Vec::new(),
                written: None,
                files: BTreeMap::new(),
                copies: Vec::new(),
            }
        }

        fn page(&self) -> String {
            String::from_utf8(self.written.clone().expect("the engine wrote an artifact"))
                .expect("the artifact is valid UTF-8")
        }

        fn file(&self, relative: &str) -> String {
            String::from_utf8(self.files.get(relative).expect(relative).clone()).expect("utf-8")
        }
    }

    impl RenderContext for FakeContext {
        fn stage_file(&mut self, _name: &str, _contents: &[u8]) -> Result<PathBuf, RenderError> {
            panic!("the html engine stages nothing");
        }

        fn run(&mut self, _invocation: &Invocation) -> Result<ToolOutput, RenderError> {
            panic!("the html engine runs no external tool");
        }

        fn write_output(&mut self, contents: &[u8]) -> Result<(), RenderError> {
            self.written = Some(contents.to_vec());
            Ok(())
        }

        fn write_file(&mut self, relative: &str, contents: &[u8]) -> Result<(), RenderError> {
            self.files.insert(relative.to_string(), contents.to_vec());
            Ok(())
        }

        fn copy_file(&mut self, from: &Path, relative: &str) -> Result<(), RenderError> {
            self.copies.push((from.to_path_buf(), relative.to_string()));
            Ok(())
        }

        fn warn(&mut self, message: &str) {
            self.warnings.push(message.to_string());
        }

        fn output_path(&self) -> &Path {
            &self.output
        }
    }

    fn doc(
        title: &str,
        frontmatter: &str,
        body: &str,
        links: Vec<ResolvedLink>,
    ) -> PreparedDocument {
        PreparedDocument {
            id: Id::from_str(ULID).expect("ulid parses"),
            path: PathBuf::from("/vault/all-notes/note.md"),
            vault_root: PathBuf::from("/vault"),
            title: title.to_string(),
            tags: vec!["area/work".to_string()],
            created: "2020-01-01".to_string(),
            frontmatter: serde_yaml_ng::from_str(frontmatter)
                .expect("the fixture frontmatter parses into a mapping"),
            body: body.to_string(),
            links,
        }
    }

    fn resolved_link(range: Range<usize>, title: &str, slug: &str) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: Id::from_str(ULID).expect("ulid parses"),
            target: Some(LinkTarget {
                title: title.to_string(),
                slug: slug.to_string(),
            }),
        }
    }

    #[test]
    fn writes_the_page_and_the_builtin_theme_beside_it() {
        let document = doc(
            "My Note",
            "priority: 2\n",
            "# Heading\n\nSome *body* text.\n\n## Second\n",
            Vec::new(),
        );
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let page = ctx.page();
        assert!(page.starts_with("<!doctype html>"), "{page}");
        assert!(page.contains("<html lang=\"en\">"), "{page}");
        assert!(page.contains("<title>My Note</title>"), "{page}");
        assert!(page.contains("<body class=\"document\">"), "{page}");
        assert!(
            page.contains("<span class=\"site-name\">My Note</span>"),
            "{page}"
        );
        assert!(
            page.contains("<link rel=\"stylesheet\" href=\"note_files/style.css\">"),
            "{page}"
        );
        assert!(
            page.contains("<script defer src=\"note_files/app.js\"></script>"),
            "{page}"
        );
        assert!(
            !page.contains("search-toggle") && !page.contains("search.js"),
            "no search on a standalone page: {page}"
        );
        assert!(!page.contains("sidebar-pane"), "{page}");
        assert!(page.contains("class=\"theme-switch\""), "{page}");
        // The outline lists the body's headings; the leading H1 stays, since
        // it does not read as the title.
        assert!(
            page.contains("<li class=\"depth-2\"><a href=\"#second\">Second</a></li>"),
            "{page}"
        );
        assert!(
            page.contains("<use href=\"#icon-tag\"/></svg>area/work</span>"),
            "{page}"
        );
        assert!(page.contains("<dt>priority</dt>\n<dd>2</dd>"), "{page}");
        assert!(page.contains("<h1 id=\"heading\">Heading</h1>"), "{page}");
        assert!(page.contains("<em>body</em>"), "{page}");
        assert!(
            page.contains("<symbol id=\"icon-tag\""),
            "the sprite is inline: {page}"
        );

        assert_eq!(ctx.file("style.css"), SiteTheme::builtin().stylesheet);
        assert_eq!(ctx.file("app.js"), frontend::app_js());
        assert!(
            ctx.files.contains_key("fonts/LICENSE"),
            "{:?}",
            ctx.files.keys()
        );
        assert!(
            ctx.files.keys().all(|file| !file.starts_with("templates/")),
            "the built-in templates are not written beside the page: {:?}",
            ctx.files.keys()
        );
        assert!(
            ctx.files.keys().all(|name| !name.starts_with("icons/")),
            "the icon files stay out: {:?}",
            ctx.files.keys()
        );
        assert!(
            ctx.files.keys().all(|name| !name.starts_with("grammars/")),
            "no code, no grammar: {:?}",
            ctx.files.keys()
        );
        assert!(ctx.copies.is_empty());
        assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
    }

    #[test]
    fn writes_the_grammars_the_code_needs_before_the_page_script() {
        let document = doc(
            "Code",
            "{}",
            "```rust\nfn x() {}\n```\n\n```no-such-language\nx\n```\n",
            Vec::new(),
        );
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");
        let page = ctx.page();
        let grammar_at = page
            .find("note_files/grammars/rust.js")
            .expect("grammar script");
        let app_at = page.find("note_files/app.js").expect("page script");
        assert!(grammar_at < app_at, "{page}");
        assert!(
            ctx.file("grammars/rust.js").contains("\"name\":\"rust\""),
            "the grammar registers itself"
        );
        assert_eq!(ctx.warnings.len(), 1, "{:?}", ctx.warnings);
        assert!(
            ctx.warnings[0].contains("no highlighting grammar for `no-such-language`"),
            "{}",
            ctx.warnings[0]
        );
    }

    #[test]
    fn mirrors_referenced_files_and_warns_about_the_rest() {
        let vault = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(vault.path().join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(vault.path().join("assets")).expect("assets");
        std::fs::write(vault.path().join("all-notes/diagram.png"), b"png").expect("diagram");
        std::fs::write(vault.path().join("assets/logo.svg"), b"svg").expect("logo");
        let mut document = doc(
            "Pictures",
            "{}",
            "![d](diagram.png) ![l](../assets/logo.svg) ![o](../../outside.png) ![m](missing.png)\n",
            Vec::new(),
        );
        document.vault_root = vault.path().to_path_buf();
        document.path = vault.path().join("all-notes/note.md");
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");
        let page = ctx.page();
        assert!(
            page.contains("<img src=\"note_files/files/all-notes/diagram.png\" alt=\"d\">"),
            "{page}"
        );
        assert!(
            page.contains("<img src=\"note_files/files/assets/logo.svg\" alt=\"l\">"),
            "{page}"
        );
        assert!(
            page.contains("<img src=\"../../outside.png\" alt=\"o\">"),
            "a path outside the vault stays as written: {page}"
        );
        let copies: Vec<&str> = ctx.copies.iter().map(|(_, to)| to.as_str()).collect();
        assert_eq!(
            copies,
            ["files/all-notes/diagram.png", "files/assets/logo.svg"]
        );
        let joined = ctx.warnings.join("\n");
        assert!(
            joined.contains("`../../outside.png` points outside the vault"),
            "{joined}"
        );
        assert!(
            joined.contains("referenced file not found in the vault")
                && joined.contains("missing.png"),
            "{joined}"
        );
        assert_eq!(ctx.warnings.len(), 2, "{joined}");
    }

    #[test]
    fn a_vault_theme_is_copied_beside_the_page_without_its_icons() {
        let theme_dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(theme_dir.path().join("fonts")).expect("fonts");
        std::fs::create_dir_all(theme_dir.path().join("icons")).expect("icons");
        std::fs::write(
            theme_dir.path().join("style.css"),
            "body { color: rebeccapurple }",
        )
        .expect("css");
        std::fs::write(theme_dir.path().join("fonts/a.woff2"), b"font").expect("font");
        std::fs::write(theme_dir.path().join("icons/tag.svg"), "<svg/>").expect("icon");
        std::fs::create_dir_all(theme_dir.path().join("templates")).expect("templates");
        std::fs::write(
            theme_dir.path().join("templates/page.html"),
            "{% extends \"ntropy/page.html\" %}{% block body_class %}themed{% endblock %}",
        )
        .expect("template");
        let settings = DocumentSettings {
            theme: Some(SiteTheme {
                name: "corporate".to_string(),
                stylesheet: "body { color: rebeccapurple }".to_string(),
                icons: BTreeMap::from([("x".to_string(), "<symbol id=\"icon-x\"/>".to_string())]),
                templates: BTreeMap::from([(
                    "page.html".to_string(),
                    "{% extends \"ntropy/page.html\" %}{% block body_class %}themed{% endblock %}"
                        .to_string(),
                )]),
            }),
            theme_dir: Some(theme_dir.path().to_path_buf()),
            lang: "de".to_string(),
        };
        let document = doc("Themed", "{}", "text\n", Vec::new());
        let mut ctx = FakeContext::new();
        Html::new(settings)
            .render(&document, &mut ctx)
            .expect("render succeeds");
        let page = ctx.page();
        assert!(page.contains("<html lang=\"de\">"), "{page}");
        assert!(
            page.contains("<symbol id=\"icon-x\"/>"),
            "the theme's sprite: {page}"
        );
        assert!(
            page.contains("<body class=\"themed\">"),
            "the theme's template renders the artifact: {page}"
        );
        let copies: Vec<&str> = ctx.copies.iter().map(|(_, to)| to.as_str()).collect();
        assert_eq!(
            copies,
            ["fonts/a.woff2", "style.css"],
            "icons and templates stay out of the files directory"
        );
        assert!(
            !ctx.files.contains_key("style.css"),
            "the built-in stylesheet is not written"
        );
        assert!(ctx.files.contains_key("app.js"));
    }

    #[test]
    fn note_links_target_the_sibling_artifact_by_slug() {
        let body = "See [x](01ARZ3NDEKTSV4RRFFQ69G5FAV-other.md) now.\n";
        let start = body.find("[x]").expect("the link is in the body");
        let end = start + "[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-other.md)".len();
        let document = doc(
            "Linker",
            "{}",
            body,
            vec![resolved_link(start..end, "The Other", "other")],
        );
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");
        assert!(
            ctx.page()
                .contains("<a class=\"note-link\" href=\"other.html\">The Other</a>"),
            "{}",
            ctx.page()
        );
    }
}
