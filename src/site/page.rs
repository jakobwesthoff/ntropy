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
    ("page.html", include_str!("templates/page.html")),
];

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

/// A page of the exported site: the chrome around a content fragment
/// (ADR 0054); or, with `document` set, the standalone page of one
/// rendered note (ADR 0057), which keeps the header, the scheme switch, and
/// the outline and has no navigation column, search, breadcrumbs, or
/// pager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub lang: String,
    /// What the header shows: the site's title, or for a standalone page the
    /// note's.
    pub site_title: String,
    pub title: String,
    /// The `../` prefix reaching the site root from this page.
    pub prefix: String,
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
    /// Whether this is a standalone rendered note rather than a site page.
    pub document: bool,
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
            icons => Value::from_safe_string(self.icons.clone()),
            sidebar => Value::from_safe_string(self.sidebar.clone()),
            breadcrumbs => self.breadcrumbs,
            outline => Value::from_safe_string(self.outline.clone()),
            prev => link(&self.prev),
            next => link(&self.next),
            body => Value::from_safe_string(self.body.clone()),
            document => self.document,
        };
        environment()
            .get_template("page.html")
            .expect("the embedded page template is registered")
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

    fn page() -> Page {
        Page {
            lang: "en".to_string(),
            site_title: "Vault & Co".to_string(),
            title: "Rust <Tips>".to_string(),
            prefix: "../".to_string(),
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
            document: false,
        }
    }

    fn document() -> Page {
        Page {
            site_title: "Quarterly <Review>".to_string(),
            title: "Quarterly <Review>".to_string(),
            prefix: String::new(),
            stylesheet: "quarterly-review_files/style.css".to_string(),
            scripts: vec!["quarterly-review_files/app.js".to_string()],
            sidebar: String::new(),
            breadcrumbs: Vec::new(),
            prev: None,
            next: None,
            document: true,
            ..page()
        }
    }

    #[test]
    fn a_standalone_page_pins_its_structure() {
        insta::assert_snapshot!(document().render());
    }

    #[test]
    fn a_standalone_page_keeps_the_header_switch_and_outline_and_drops_the_rest() {
        let out = document().render();
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
        .render();
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
        let out = fragment.render();
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
