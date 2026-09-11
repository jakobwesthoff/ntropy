// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Page templates: minijinja templates embedded in the binary and the typed
//! contexts they render from (ADR 0050).
//!
//! Templates are real files under `src/site/templates/`, embedded with
//! `include_str!` like the vault seed content, so they are edited as HTML.
//! minijinja escapes every value it interpolates; the values that are HTML
//! already (a converted body, a stylesheet, a frontmatter value) are marked
//! safe where the context is built, never inside a template.

use minijinja::Environment;
use minijinja::value::Value;
use serde::Serialize;

use crate::render::html::frontmatter::Field;
use crate::render::html::writer::escape;

/// The embedded templates, by the name templates refer to each other with.
const TEMPLATES: &[(&str, &str)] = &[
    ("base.html", include_str!("templates/base.html")),
    ("note.html", include_str!("templates/note.html")),
    ("document.html", include_str!("templates/document.html")),
    ("page.html", include_str!("templates/page.html")),
];

/// A note's header and body as one fragment, the content of a note page in
/// the site and of the standalone document alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteFragment {
    pub title: String,
    pub created: String,
    pub tags: Vec<String>,
    pub fields: Vec<Field>,
    /// The converted note body, an HTML fragment.
    pub body: String,
}

impl NoteFragment {
    pub fn render(&self) -> String {
        environment()
            .get_template("note.html")
            .expect("the embedded note template is registered")
            .render(note_context(self))
            .expect("the embedded templates render any well-typed context")
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

/// A link the page template renders: a breadcrumb, a previous or next
/// neighbour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageLink {
    pub label: String,
    pub href: String,
}

/// A page of the exported site: the chrome around a content fragment
/// (ADR 0054).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub lang: String,
    pub site_title: String,
    pub title: String,
    /// The `../` prefix reaching the site root from this page.
    pub prefix: String,
    /// The stylesheet's `href`, relative to this page.
    pub stylesheet: String,
    /// The `src` of every script the page loads, relative to this page: the
    /// page script and the grammars its code blocks need.
    pub scripts: Vec<String>,
    /// The sidebar, an HTML fragment.
    pub sidebar: String,
    pub breadcrumbs: Vec<PageLink>,
    /// The outline, an HTML fragment, or empty.
    pub outline: String,
    pub prev: Option<PageLink>,
    pub next: Option<PageLink>,
    /// The page content, an HTML fragment.
    pub body: String,
}

impl Page {
    pub fn render(&self) -> String {
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
            prefix => self.prefix,
            stylesheet => self.stylesheet,
            scripts => self.scripts,
            sidebar => Value::from_safe_string(self.sidebar.clone()),
            breadcrumbs => self.breadcrumbs,
            outline => Value::from_safe_string(self.outline.clone()),
            prev => link(&self.prev),
            next => link(&self.next),
            body => Value::from_safe_string(self.body.clone()),
        };
        environment()
            .get_template("page.html")
            .expect("the embedded page template is registered")
            .render(context)
            .expect("the embedded templates render any well-typed context")
    }
}

/// A standalone single-note page, the artifact of `render --to html`
/// (ADR 0046): the stylesheet inlined, the note's header and body, no site
/// chrome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Document {
    pub lang: String,
    pub title: String,
    /// The creation date as the CLI shows it.
    pub created: String,
    pub tags: Vec<String>,
    pub fields: Vec<Field>,
    /// The theme's stylesheet, inlined into the page.
    pub stylesheet: String,
    /// The converted note body, an HTML fragment.
    pub body: String,
}

impl Document {
    /// Render the page.
    pub fn render(&self) -> String {
        let note = NoteFragment {
            title: self.title.clone(),
            created: self.created.clone(),
            tags: self.tags.clone(),
            fields: self.fields.clone(),
            body: self.body.clone(),
        };
        let context = minijinja::context! {
            lang => self.lang,
            stylesheet => Value::from_safe_string(self.stylesheet.clone()),
            ..note_context(&note)
        };
        environment()
            .get_template("document.html")
            .expect("the embedded document template is registered")
            .render(context)
            .expect("the embedded templates render any well-typed context")
    }
}

/// The environment holding every embedded template. Built per call: the
/// templates are static strings, so parsing them is the whole cost, and a
/// render happens once per page.
fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_formatter(formatter);
    for (name, source) in TEMPLATES {
        env.add_template(name, source)
            .expect("the embedded templates are valid at build time");
    }
    env
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

    fn document() -> Document {
        Document {
            lang: "en".to_string(),
            title: "Quarterly <Review>".to_string(),
            created: "2026-06-24".to_string(),
            tags: vec!["area/work".to_string(), "a&b".to_string()],
            fields: vec![Field {
                key: "status".to_string(),
                html: "<ul><li>draft</li></ul>".to_string(),
            }],
            stylesheet: "body { color: red }".to_string(),
            body: "<p>Hello <em>there</em></p>\n".to_string(),
        }
    }

    #[test]
    fn a_document_page_pins_its_structure() {
        insta::assert_snapshot!(document().render());
    }

    #[test]
    fn text_values_are_escaped_and_html_values_are_not() {
        let page = document().render();
        assert!(page.contains("<title>Quarterly &lt;Review&gt;</title>"));
        assert!(page.contains("<h1 class=\"note-title\">Quarterly &lt;Review&gt;</h1>"));
        assert!(page.contains("<li class=\"tag\">area/work</li>"));
        assert!(page.contains("<li class=\"tag\">a&amp;b</li>"));
        assert!(page.contains("<dd><ul><li>draft</li></ul></dd>"));
        assert!(page.contains("<p>Hello <em>there</em></p>"));
        assert!(page.contains("<style>\nbody { color: red }\n</style>"));
        assert!(page.contains("<html lang=\"en\">"));
    }

    #[test]
    fn empty_tags_and_fields_leave_no_list_behind() {
        let page = Document {
            tags: Vec::new(),
            fields: Vec::new(),
            ..document()
        }
        .render();
        assert!(!page.contains("class=\"tags\""), "{page}");
        assert!(!page.contains("class=\"frontmatter\""), "{page}");
    }

    fn page() -> Page {
        Page {
            lang: "en".to_string(),
            site_title: "Vault & Co".to_string(),
            title: "Rust <Tips>".to_string(),
            prefix: "../".to_string(),
            stylesheet: "../assets/style.css".to_string(),
            scripts: vec![
                "../assets/app.js".to_string(),
                "../assets/grammars/rust.js".to_string(),
            ],
            sidebar: "<nav class=\"sidebar\">S</nav>\n".to_string(),
            breadcrumbs: vec![
                PageLink {
                    label: "by-status".to_string(),
                    href: "../views/by-status/index.html".to_string(),
                },
                PageLink {
                    label: "done".to_string(),
                    href: "../views/by-status/done/index.html".to_string(),
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

    #[test]
    fn a_site_page_pins_its_structure() {
        insta::assert_snapshot!(page().render());
    }

    #[test]
    fn a_site_page_escapes_text_and_splices_fragments() {
        let out = page().render();
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
            out.contains(
                "<a class=\"prev\" rel=\"prev\" href=\"earlier.html\">Earlier &lt;one&gt;</a>"
            ),
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
        assert!(out.contains("<button class=\"theme-toggle\""), "{out}");
        assert!(
            out.contains("<li><a href=\"../views/by-status/done/index.html\">done</a></li>"),
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
        .render();
        assert!(!out.contains("class=\"breadcrumbs\""), "{out}");
        assert!(!out.contains("class=\"pager\""), "{out}");
    }

    #[test]
    fn a_note_fragment_renders_alone() {
        let fragment = NoteFragment {
            title: "T".to_string(),
            created: "2026-01-01".to_string(),
            tags: vec![],
            fields: vec![],
            body: "<p>b</p>\n".to_string(),
        };
        let out = fragment.render();
        assert!(out.starts_with("<header class=\"note-header\">"), "{out}");
        assert!(out.contains("<p>b</p>"), "{out}");
    }

    #[test]
    fn every_embedded_template_parses() {
        // The environment panics on a template that does not parse; building
        // it is the test.
        let env = environment();
        for (name, _) in TEMPLATES {
            env.get_template(name).expect("registered");
        }
    }
}
