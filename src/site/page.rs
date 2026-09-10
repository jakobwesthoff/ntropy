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

/// The embedded templates, by the name templates refer to each other with.
const TEMPLATES: &[(&str, &str)] = &[
    ("base.html", include_str!("templates/base.html")),
    ("note.html", include_str!("templates/note.html")),
    ("document.html", include_str!("templates/document.html")),
];

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
        let context = minijinja::context! {
            lang => self.lang,
            title => self.title,
            created => self.created,
            tags => self.tags,
            fields => self.fields.iter().map(|field| minijinja::context! {
                key => field.key,
                html => Value::from_safe_string(field.html.clone()),
            }).collect::<Vec<_>>(),
            stylesheet => Value::from_safe_string(self.stylesheet.clone()),
            body => Value::from_safe_string(self.body.clone()),
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
    for (name, source) in TEMPLATES {
        env.add_template(name, source)
            .expect("the embedded templates are valid at build time");
    }
    env
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
