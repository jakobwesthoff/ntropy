// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A note's frontmatter as the fields a page header shows (ADR 0054).
//!
//! The page shows the title prominently and the tags as chips, so those two
//! fields are left out here; every other field becomes a key and an HTML
//! value, nested values included. A field whose value carries nothing (null,
//! an empty string, an empty list or mapping) is left out too, which is what
//! the Typst engine's default template does. Hiding further fields is a
//! theme's job through CSS.

use serde::Serialize;
use serde_yaml_ng::{Mapping, Value};

use super::writer::HtmlWriter;

/// One frontmatter field for the page header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub key: String,
    /// The value as an HTML fragment, spliced unescaped by the template.
    pub html: String,
}

/// The fields a page header shows, in the frontmatter's own order. The
/// title and the tags have their own place in the header, and the `site`
/// table is navigation settings (ADR 0056), not content.
pub fn fields(frontmatter: &Mapping) -> Vec<Field> {
    frontmatter
        .iter()
        .filter(|(key, value)| {
            let key = scalar_text(key);
            key != "title" && key != "tags" && key != "site" && !is_empty(value)
        })
        .map(|(key, value)| Field {
            key: scalar_text(key),
            html: value_html(value),
        })
        .collect()
}

/// Whether a value carries nothing worth showing.
fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Sequence(items) => items.is_empty(),
        Value::Mapping(map) => map.is_empty(),
        _ => false,
    }
}

/// A value as an HTML fragment: scalars as escaped text, a sequence as a
/// list, a mapping as a definition list, nesting as it nests.
pub fn value_html(value: &Value) -> String {
    let mut writer = HtmlWriter::new();
    write_value(&mut writer, value);
    writer.finish()
}

fn write_value(writer: &mut HtmlWriter, value: &Value) {
    match value {
        Value::Sequence(items) => {
            writer.syntax("<ul>");
            for item in items {
                writer.syntax("<li>");
                write_value(writer, item);
                writer.syntax("</li>");
            }
            writer.syntax("</ul>");
        }
        Value::Mapping(map) => {
            writer.syntax("<dl>");
            for (key, item) in map {
                writer.syntax("<dt>");
                writer.text(&scalar_text(key));
                writer.syntax("</dt><dd>");
                write_value(writer, item);
                writer.syntax("</dd>");
            }
            writer.syntax("</dl>");
        }
        other => writer.text(&scalar_text(other)),
    }
}

/// A scalar's text: strings as they are, numbers and booleans in their YAML
/// spelling, null as nothing, a tagged value with its tag (`!degrees 90`),
/// and a collection used as a key in its YAML spelling.
fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => serde_yaml_ng::to_string(other)
            .expect("serializing an in-memory YAML value cannot fail")
            .trim_end()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Mapping {
        serde_yaml_ng::from_str(yaml).expect("the fixture parses into a mapping")
    }

    #[test]
    fn title_tags_site_and_empty_values_are_left_out() {
        let fields = fields(&parse(
            "title: T\ntags: [a]\nsite: {order: 1}\npriority: 2\nreviewer: null\nblank: \"\"\naliases: []\nmeta: {}\n",
        ));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].key, "priority");
        assert_eq!(fields[0].html, "2");
    }

    #[test]
    fn fields_keep_the_frontmatter_order() {
        let fields = fields(&parse("zeta: 1\nalpha: 2\n"));
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, ["zeta", "alpha"]);
    }

    #[test]
    fn scalars_render_as_escaped_text() {
        assert_eq!(value_html(&Value::from("a <b> & c")), "a &lt;b&gt; &amp; c");
        assert_eq!(value_html(&Value::from(1.5)), "1.5");
        assert_eq!(value_html(&Value::from(false)), "false");
        assert_eq!(value_html(&Value::Null), "");
    }

    #[test]
    fn sequences_and_mappings_nest() {
        let map = parse("review:\n  due: 2026-08-01\n  by: [jakob, <x>]\n");
        let review = map.get("review").expect("field");
        assert_eq!(
            value_html(review),
            "<dl><dt>due</dt><dd>2026-08-01</dd><dt>by</dt><dd><ul><li>jakob</li><li>&lt;x&gt;</li></ul></dd></dl>"
        );
    }

    #[test]
    fn a_tagged_value_keeps_its_tag_as_text() {
        let map = parse("angle: !degrees 90\n");
        assert_eq!(value_html(map.get("angle").expect("field")), "!degrees 90");
    }

    #[test]
    fn a_non_string_key_uses_its_yaml_spelling() {
        let fields = fields(&parse("1: one\ntrue: yes\n"));
        let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, ["1", "true"]);
    }
}
