// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Page templates: the minijinja templates a theme renders pages with and
//! the typed contexts they render from (ADR 0050, ADR 0058).
//!
//! The built-in templates are real files under `src/site/theme/templates/`,
//! embedded with the rest of the built-in theme, so they are edited as
//! HTML and `site theme init` writes them out. A vault theme's
//! `templates/*.html` replace them by name; every built-in template stays
//! reachable as `ntropy/<name>` so a theme template can extend one and
//! override single blocks. minijinja escapes every value it interpolates;
//! the values that are HTML already (a converted body, a stylesheet, a
//! frontmatter value) are marked safe where the context is built, never
//! inside a template.

use minijinja::Environment;
use minijinja::value::Value;
use serde::Serialize;

use super::theme::{self, SiteTheme};
use crate::render::RenderError;
use crate::render::html::frontmatter::Field;
use crate::render::html::writer::escape;

/// The prefix under which every built-in template is registered beside
/// its plain name, for a theme template to extend or include.
pub const BUILTIN_NAMESPACE: &str = "ntropy/";

/// The template every page renders with unless a note names another.
pub const PAGE_TEMPLATE: &str = "page.html";

/// The template of a note's header and body.
pub const NOTE_TEMPLATE: &str = "note.html";

/// The templates of one theme, parsed and ready to render: the theme's own
/// under their names, the built-in ones under the names the theme leaves
/// free and, all of them, under [`BUILTIN_NAMESPACE`]. Built once per
/// export; parsing is the whole cost.
#[derive(Debug)]
pub struct Templates {
    env: Environment<'static>,
}

impl Templates {
    /// The environment for `theme`. Every template is parsed here, so a
    /// theme template with a syntax error fails before any page is
    /// written, naming the template.
    pub fn new(theme: &SiteTheme) -> Result<Templates, RenderError> {
        let mut env = Environment::new();
        env.set_formatter(formatter);
        let add = |env: &mut Environment<'static>, name: String, source: String| {
            env.add_template_owned(name.clone(), source)
                .map_err(|error| RenderError::ThemeTemplate {
                    name,
                    reason: error.to_string(),
                })
        };
        for (name, source) in theme::builtin_templates() {
            add(
                &mut env,
                format!("{BUILTIN_NAMESPACE}{name}"),
                source.clone(),
            )?;
            if !theme.templates.contains_key(name) {
                add(&mut env, name.clone(), source.clone())?;
            }
        }
        for (name, source) in &theme.templates {
            add(&mut env, name.clone(), source.clone())?;
        }
        Ok(Templates { env })
    }

    /// The built-in theme's templates alone.
    pub fn builtin() -> Templates {
        Templates::new(&SiteTheme::builtin()).expect("the built-in templates parse")
    }

    /// Whether a template of that name exists, the theme's or built-in.
    pub fn has(&self, name: &str) -> bool {
        self.env.get_template(name).is_ok()
    }

    /// Render the template `name` with `context`.
    fn render(&self, name: &str, context: Value) -> Result<String, RenderError> {
        let template = self
            .env
            .get_template(name)
            .map_err(|error| RenderError::ThemeTemplate {
                name: name.to_string(),
                reason: error.to_string(),
            })?;
        template
            .render(context)
            .map_err(|error| RenderError::ThemeTemplate {
                name: name.to_string(),
                reason: error.to_string(),
            })
    }
}

/// A tag in a note's header: on a site page it links to the tag's page, in
/// the standalone document it is plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagLink {
    pub label: String,
    pub href: Option<String>,
}

impl TagLink {
    /// The tags of a standalone document: labels without links.
    pub fn plain(tags: &[String]) -> Vec<TagLink> {
        tags.iter()
            .map(|tag| TagLink {
                label: tag.clone(),
                href: None,
            })
            .collect()
    }
}

/// A note's header and body as one fragment, the content of a note page in
/// the site and of the standalone document alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteFragment {
    pub title: String,
    pub created: String,
    pub tags: Vec<TagLink>,
    pub fields: Vec<Field>,
    /// The converted note body, an HTML fragment.
    pub body: String,
}

impl NoteFragment {
    pub fn render(&self, templates: &Templates) -> Result<String, RenderError> {
        templates.render(NOTE_TEMPLATE, note_context(self))
    }
}

fn note_context(note: &NoteFragment) -> Value {
    minijinja::context! {
        title => note.title,
        created => note.created,
        tags => note.tags,
        fields => note.fields.iter().map(|field| minijinja::context! {
            key => field.key,
            html => Value::from_safe_string(field.html.clone()),
        }).collect::<Vec<_>>(),
        body => Value::from_safe_string(note.body.clone()),
    }
}

/// A link the page template renders: a previous or next neighbour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageLink {
    pub label: String,
    pub href: String,
}

/// One step of a page's breadcrumb; a step without a page of its own (a
/// hand-assembled nav group, ADR 0056) has no `href` and renders as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Crumb {
    pub label: String,
    pub href: Option<String>,
}

/// What a page is rendered from, the `kind` every template receives
/// (ADR 0058).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    /// The front page: the index note, or the generated overview.
    Front,
    /// A note's own page.
    Note,
    /// A tag, view, or group page with its listing, a landing note's page
    /// among them, and a section's index.
    Group,
    /// The standalone `render --to html` artifact (ADR 0057), which keeps
    /// the header, the scheme switch, and the outline and has no navigation
    /// column, search, breadcrumbs, or pager.
    Document,
}

impl PageKind {
    /// The kind as the template sees it.
    pub fn name(self) -> &'static str {
        match self {
            PageKind::Front => "front",
            PageKind::Note => "note",
            PageKind::Group => "group",
            PageKind::Document => "document",
        }
    }
}

/// The note a page is rendered from, as the template sees it: the raw
/// frontmatter mapping beside the lifted fields, so a theme template can
/// read a field of its own (a tagline, a hero image) from the note.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteContext {
    pub id: String,
    pub title: String,
    pub created: String,
    pub tags: Vec<String>,
    pub frontmatter: serde_yaml_ng::Mapping,
}

/// A page of the exported site: the chrome around a content fragment
/// (ADR 0054), or the standalone page of one rendered note (ADR 0057);
/// `kind` tells the template which.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub lang: String,
    /// What the header shows: the site's title, or for a standalone page the
    /// note's.
    pub site_title: String,
    pub title: String,
    /// The page's site-relative path, `notes/<slug>.html`; the artifact's
    /// file name for a standalone page.
    pub path: String,
    /// The `../` prefix reaching the site root from this page.
    pub prefix: String,
    pub kind: PageKind,
    /// The note the page is rendered from, or `None` for a page made of a
    /// listing alone.
    pub note: Option<NoteContext>,
    /// The `[site.vars]` table of the vault config, for the theme's
    /// templates; the built-in ones read nothing from it.
    pub vars: toml::Table,
    /// The sidebar as data, beside its rendered form.
    pub nav: Vec<super::nav::NavSectionData>,
    /// The stylesheet's `href`, relative to this page.
    pub stylesheet: String,
    /// The `src` of every script the page loads, relative to this page: the
    /// page script and the grammars its code blocks need.
    pub scripts: Vec<String>,
    /// The theme's icon sprite, inlined once per page.
    pub icons: String,
    /// The sidebar, an HTML fragment.
    pub sidebar: String,
    pub breadcrumbs: Vec<Crumb>,
    /// The outline, an HTML fragment, or empty.
    pub outline: String,
    pub prev: Option<PageLink>,
    pub next: Option<PageLink>,
    /// The page content, an HTML fragment.
    pub body: String,
}

impl Page {
    pub fn render(&self, templates: &Templates) -> Result<String, RenderError> {
        let link = |link: &Option<PageLink>| {
            link.as_ref().map(|link| {
                minijinja::context! {
                    title => link.label,
                    href => link.href,
                }
            })
        };
        let context = minijinja::context! {
            lang => self.lang,
            site_title => self.site_title,
            title => self.title,
            path => self.path,
            prefix => self.prefix,
            kind => self.kind.name(),
            note => self.note,
            vars => self.vars,
            nav => self.nav,
            stylesheet => self.stylesheet,
            scripts => self.scripts,
            icons => Value::from_safe_string(self.icons.clone()),
            sidebar => Value::from_safe_string(self.sidebar.clone()),
            breadcrumbs => self.breadcrumbs,
            outline => Value::from_safe_string(self.outline.clone()),
            prev => link(&self.prev),
            next => link(&self.next),
            body => Value::from_safe_string(self.body.clone()),
        };
        templates.render(PAGE_TEMPLATE, context)
    }
}

/// Interpolation escaping: the same five characters the HTML writer
/// escapes, so a value reads identically whether the emitter or a template
/// wrote it. minijinja's own HTML escaping also encodes `/`, which would turn
/// every relative `href` into entity soup. Values marked safe pass through.
fn formatter(
    out: &mut minijinja::Output,
    state: &minijinja::State,
    value: &Value,
) -> Result<(), minijinja::Error> {
    if value.is_safe() || !matches!(state.auto_escape(), minijinja::AutoEscape::Html) {
        return minijinja::escape_formatter(out, state, value);
    }
    if value.is_undefined() {
        return Ok(());
    }
    write!(out, "{}", escape(&value.to_string())).map_err(minijinja::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Page {
        Page {
            lang: "en".to_string(),
            site_title: "Vault & Co".to_string(),
            title: "Rust <Tips>".to_string(),
            path: "notes/rust-tips.html".to_string(),
            prefix: "../".to_string(),
            kind: PageKind::Note,
            note: Some(NoteContext {
                id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
                title: "Rust <Tips>".to_string(),
                created: "2016-07-31".to_string(),
                tags: vec!["programming/rust".to_string()],
                frontmatter: serde_yaml_ng::from_str(
                    "title: Rust <Tips>\ntags: [programming/rust]\ntagline: Fast & safe\nsite:\n  order: 2\n",
                )
                .expect("the fixture frontmatter parses"),
            }),
            vars: toml::from_str("github = \"https://github.com/x/y\"\n[links]\nhome = \"/\"\n")
                .expect("the fixture vars parse"),
            nav: vec![super::super::nav::NavSectionData {
                label: "by-status".to_string(),
                href: Some("../views/by-status/index.html".to_string()),
                cloud: false,
                open: true,
                items: vec![super::super::nav::NavItemData {
                    kind: "note",
                    label: "Rust <Tips>".to_string(),
                    href: Some("../notes/rust-tips.html".to_string()),
                    current: true,
                    open: false,
                    count: None,
                    items: Vec::new(),
                }],
            }],
            stylesheet: "../assets/style.css".to_string(),
            scripts: vec![
                "../assets/grammars/rust.js".to_string(),
                "../assets/app.js".to_string(),
            ],
            icons: "<svg hidden><symbol id=\"icon-menu\"/></svg>".to_string(),
            sidebar: "<nav class=\"sidebar\">S</nav>\n".to_string(),
            breadcrumbs: vec![
                Crumb {
                    label: "Reference".to_string(),
                    href: None,
                },
                Crumb {
                    label: "by-status".to_string(),
                    href: Some("../views/by-status/index.html".to_string()),
                },
                Crumb {
                    label: "done".to_string(),
                    href: Some("../views/by-status/done/index.html".to_string()),
                },
            ],
            outline: "<nav class=\"outline\">O</nav>\n".to_string(),
            prev: Some(PageLink {
                label: "Earlier <one>".to_string(),
                href: "earlier.html".to_string(),
            }),
            next: None,
            body: "<article>B</article>\n".to_string(),
        }
    }

    fn document() -> Page {
        Page {
            site_title: "Quarterly <Review>".to_string(),
            title: "Quarterly <Review>".to_string(),
            path: "quarterly-review.html".to_string(),
            prefix: String::new(),
            kind: PageKind::Document,
            nav: Vec::new(),
            stylesheet: "quarterly-review_files/style.css".to_string(),
            scripts: vec!["quarterly-review_files/app.js".to_string()],
            sidebar: String::new(),
            breadcrumbs: Vec::new(),
            prev: None,
            next: None,
            ..page()
        }
    }

    #[test]
    fn a_standalone_page_pins_its_structure() {
        insta::assert_snapshot!(document().render(&Templates::builtin()).expect("renders"));
    }

    #[test]
    fn a_standalone_page_keeps_the_header_switch_and_outline_and_drops_the_rest() {
        let out = document().render(&Templates::builtin()).expect("renders");
        assert!(out.contains("<body class=\"document\">"), "{out}");
        assert!(
            out.contains("<span class=\"site-name\">Quarterly &lt;Review&gt;</span>"),
            "{out}"
        );
        assert!(out.contains("class=\"theme-switch\""), "{out}");
        assert!(out.contains("<nav class=\"outline\">O</nav>"), "{out}");
        assert!(!out.contains("nav-open"), "{out}");
        assert!(!out.contains("search-toggle"), "{out}");
        assert!(!out.contains("sidebar-pane"), "{out}");
        assert!(!out.contains("class=\"breadcrumbs\""), "{out}");
        assert!(!out.contains("class=\"pager\""), "{out}");
        assert!(
            out.contains("<link rel=\"stylesheet\" href=\"quarterly-review_files/style.css\">"),
            "{out}"
        );
    }

    #[test]
    fn a_site_page_pins_its_structure() {
        insta::assert_snapshot!(page().render(&Templates::builtin()).expect("renders"));
    }

    #[test]
    fn a_site_page_escapes_text_and_splices_fragments() {
        let out = page().render(&Templates::builtin()).expect("renders");
        assert!(
            out.contains("<title>Rust &lt;Tips&gt; · Vault &amp; Co</title>"),
            "{out}"
        );
        assert!(
            out.contains("<link rel=\"stylesheet\" href=\"../assets/style.css\">"),
            "{out}"
        );
        assert!(out.contains("<nav class=\"sidebar\">S</nav>"), "{out}");
        assert!(
            out.contains("<a class=\"prev\" rel=\"prev\" href=\"earlier.html\">"),
            "{out}"
        );
        assert!(!out.contains("class=\"next\""), "{out}");
        assert!(
            out.contains("<script defer src=\"../assets/app.js\"></script>"),
            "{out}"
        );
        assert!(
            out.contains("<script defer src=\"../assets/grammars/rust.js\"></script>"),
            "{out}"
        );
        assert!(
            out.contains("localStorage.getItem(\"ntropy-theme\")"),
            "the early theme script is inlined: {out}"
        );
        assert!(
            out.contains(
                "<button class=\"theme-choice\" type=\"button\" data-theme-choice=\"dark\""
            ),
            "{out}"
        );
        assert!(out.starts_with("<!doctype html>"), "{out}");
        assert_eq!(
            out.matches("<svg hidden><symbol id=\"icon-menu\"/></svg>")
                .count(),
            1,
            "the given sprite is inlined once: {out}"
        );
        assert!(
            out.contains("<span class=\"pager-title\">Earlier &lt;one&gt;</span>"),
            "{out}"
        );
        assert!(
            out.contains("<li><svg class=\"icon sep\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../views/by-status/done/index.html\">done</a></li>"),
            "{out}"
        );
        assert!(
            out.contains("<li><span>Reference</span></li>"),
            "the first crumb has no separator, and a crumb without a page is text: {out}"
        );
        assert!(
            out.contains("<li><svg class=\"icon sep\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../views/by-status/index.html\">by-status</a></li>"),
            "{out}"
        );
    }

    #[test]
    fn a_page_without_crumbs_or_neighbours_omits_those_navs() {
        let out = Page {
            breadcrumbs: Vec::new(),
            prev: None,
            next: None,
            ..page()
        }
        .render(&Templates::builtin())
        .expect("renders");
        assert!(!out.contains("class=\"breadcrumbs\""), "{out}");
        assert!(!out.contains("class=\"pager\""), "{out}");
    }

    #[test]
    fn a_note_fragment_renders_alone_and_links_its_tags_when_given_hrefs() {
        let fragment = NoteFragment {
            title: "T".to_string(),
            created: "2026-01-01".to_string(),
            tags: vec![
                TagLink {
                    label: "a/b".to_string(),
                    href: Some("../tags/a/b/index.html".to_string()),
                },
                TagLink {
                    label: "plain".to_string(),
                    href: None,
                },
            ],
            fields: vec![],
            body: "<p>b</p>\n".to_string(),
        };
        let out = fragment.render(&Templates::builtin()).expect("renders");
        assert!(out.starts_with("<header class=\"note-header\">"), "{out}");
        assert!(out.contains("<p>b</p>"), "{out}");
        assert!(
            out.contains(
                "<a class=\"tag\" href=\"../tags/a/b/index.html\"><svg class=\"icon\" aria-hidden=\"true\"><use href=\"#icon-tag\"/></svg>a/b</a>"
            ),
            "{out}"
        );
        assert!(
            out.contains("<span class=\"tag\"><svg class=\"icon\" aria-hidden=\"true\"><use href=\"#icon-tag\"/></svg>plain</span>"),
            "{out}"
        );
    }

    /// A theme with the given templates, nothing else of its own.
    fn theme_with(templates: &[(&str, &str)]) -> SiteTheme {
        SiteTheme {
            templates: templates
                .iter()
                .map(|(name, source)| (name.to_string(), source.to_string()))
                .collect(),
            ..SiteTheme::builtin()
        }
    }

    #[test]
    fn the_builtin_templates_are_registered_plain_and_under_the_namespace() {
        let templates = Templates::builtin();
        for name in ["base.html", "page.html", "note.html"] {
            assert!(templates.has(name), "{name}");
            assert!(templates.has(&format!("ntropy/{name}")), "ntropy/{name}");
        }
        assert!(!templates.has("no-such-template.html"));
    }

    #[test]
    fn a_theme_template_replaces_the_builtin_of_its_name() {
        let templates = Templates::new(&theme_with(&[(
            "page.html",
            "<p>{{ title }} in {{ site_title }}</p>",
        )]))
        .expect("parses");
        let out = page().render(&templates).expect("renders");
        assert_eq!(out, "<p>Rust &lt;Tips&gt; in Vault &amp; Co</p>");
        // The note fragment still comes from the built-in template.
        assert!(templates.has("note.html"));
    }

    #[test]
    fn a_theme_template_extends_the_builtin_and_overrides_one_block() {
        let templates = Templates::new(&theme_with(&[(
            "page.html",
            "{% extends \"ntropy/page.html\" %}{% block title %}Custom · {{ title }}{% endblock %}",
        )]))
        .expect("parses");
        let out = page().render(&templates).expect("renders");
        assert!(
            out.contains("<title>Custom · Rust &lt;Tips&gt;</title>"),
            "{out}"
        );
        assert!(
            out.contains("<nav class=\"sidebar\">S</nav>"),
            "the rest is the built-in page: {out}"
        );
    }

    #[test]
    fn a_theme_template_includes_another_theme_template() {
        let templates = Templates::new(&theme_with(&[
            ("page.html", "{% include \"partials/footer.html\" %}"),
            ("partials/footer.html", "<footer>{{ site_title }}</footer>"),
        ]))
        .expect("parses");
        let out = page().render(&templates).expect("renders");
        assert_eq!(out, "<footer>Vault &amp; Co</footer>");
    }

    #[test]
    fn a_theme_template_that_does_not_parse_fails_naming_it() {
        let err = Templates::new(&theme_with(&[("page.html", "{% if %}")]))
            .expect_err("a syntax error is refused");
        match err {
            RenderError::ThemeTemplate { name, reason } => {
                assert_eq!(name, "page.html");
                assert!(reason.contains("syntax error"), "{reason}");
            }
            other => panic!("expected ThemeTemplate, got {other:?}"),
        }
    }

    #[test]
    fn a_theme_template_that_fails_while_rendering_names_itself() {
        let templates = Templates::new(&theme_with(&[(
            "page.html",
            "{{ title | no_such_filter }}",
        )]))
        .expect("an unknown filter is found at render time");
        let err = page()
            .render(&templates)
            .expect_err("the unknown filter fails the render");
        match err {
            RenderError::ThemeTemplate { name, reason } => {
                assert_eq!(name, "page.html");
                assert!(reason.contains("no_such_filter"), "{reason}");
            }
            other => panic!("expected ThemeTemplate, got {other:?}"),
        }
    }

    #[test]
    fn a_template_reads_the_kind_path_note_vars_and_nav() {
        let templates = Templates::new(&theme_with(&[(
            "page.html",
            "{{ kind }}|{{ path }}|{{ note.id }}|{{ note.title }}|{{ note.created }}|{{ note.tags[0] }}|{{ note.frontmatter.tagline }}|{{ note.frontmatter.site.order }}|{{ vars.github }}|{{ vars.links.home }}|{{ nav[0].label }}|{{ nav[0].open }}|{{ nav[0].items[0].current }}|{{ nav[0].items[0].kind }}",
        )]))
        .expect("parses");
        assert_eq!(
            page().render(&templates).expect("renders"),
            "note|notes/rust-tips.html|01ARZ3NDEKTSV4RRFFQ69G5FAV|Rust &lt;Tips&gt;|2016-07-31|programming/rust|Fast &amp; safe|2|https://github.com/x/y|/|by-status|True|True|note"
        );
    }

    #[test]
    fn a_page_from_no_note_leaves_note_undefined() {
        let templates = Templates::new(&theme_with(&[(
            "page.html",
            "{% if note %}{{ note.title }}{% else %}no note{% endif %}|{{ kind }}",
        )]))
        .expect("parses");
        let listing = Page {
            note: None,
            kind: PageKind::Group,
            ..page()
        };
        assert_eq!(
            listing.render(&templates).expect("renders"),
            "no note|group"
        );
    }

    /// The built-in page exposes one empty block at every seam, in the
    /// order the seams appear; a theme fills them without copying the file.
    #[test]
    fn the_builtin_page_exposes_its_blocks_in_order() {
        let templates = Templates::new(&theme_with(&[(
            "page.html",
            concat!(
                "{% extends \"ntropy/page.html\" %}",
                "{% block head %}{{ super() }}<!--HEAD-->{% endblock %}",
                "{% block header_nav %}<!--HEADER-NAV-->{% endblock %}",
                "{% block header_tools %}<!--HEADER-TOOLS-->{% endblock %}",
                "{% block before_content %}<!--BEFORE-->{% endblock %}",
                "{% block after_content %}<!--AFTER-->{% endblock %}",
                "{% block footer %}<!--FOOTER-->{% endblock %}",
                "{% block scripts %}<!--SCRIPTS-->{% endblock %}",
            ),
        )]))
        .expect("parses");
        let out = page().render(&templates).expect("renders");
        let at = |needle: &str| {
            out.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {out}"))
        };
        let order = [
            "<link rel=\"stylesheet\"",
            "<!--HEAD-->",
            "</head>",
            "class=\"site-name\"",
            "<!--HEADER-NAV-->",
            "class=\"search-toggle\"",
            "<!--HEADER-TOOLS-->",
            "class=\"theme-switch\"",
            "<main>",
            "<!--BEFORE-->",
            "class=\"breadcrumbs\"",
            "class=\"content\"",
            "class=\"pager\"",
            "<!--AFTER-->",
            "</main>",
            "<!--FOOTER-->",
            "<!--SCRIPTS-->",
            "</body>",
        ];
        for pair in order.windows(2) {
            assert!(
                at(pair[0]) < at(pair[1]),
                "{} comes before {}: {out}",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(out.matches("<!--HEAD-->").count(), 1);
        // The same page without a theme renders no block content at all.
        let plain = page().render(&Templates::builtin()).expect("renders");
        assert!(!plain.contains("<!--"), "{plain}");
    }

    #[test]
    fn a_theme_note_template_renders_the_fragment() {
        let templates = Templates::new(&theme_with(&[(
            "note.html",
            "<h1>{{ title }}</h1>{{ body }}",
        )]))
        .expect("parses");
        let fragment = NoteFragment {
            title: "T".to_string(),
            created: "2026-01-01".to_string(),
            tags: vec![],
            fields: vec![],
            body: "<p>b</p>\n".to_string(),
        };
        assert_eq!(
            fragment.render(&templates).expect("renders"),
            "<h1>T</h1><p>b</p>\n"
        );
    }
}
