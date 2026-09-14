// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The search data a site embeds (ADR 0052): every exported note with what
//! the browser-side query evaluator needs, as one classic script that
//! assigns `window.__ntropySearch`. The page script loads it on demand, so
//! a page costs nothing until a reader searches.
//!
//! The frontmatter travels as JSON the way the CLI's evaluator sees it:
//! only string keys, since a `field:` predicate looks a field up by its
//! string name and a non-string key can never match; tagged values as
//! `null`, since they have no scalar form and never match either.
//!
//! Beside the notes, the data lists the site's own pages a search can land
//! on: each section's index, every tag page, and every view group page,
//! with the note count under it, so a search for a tag finds the tag's
//! page as well as the notes carrying it.

use serde_yaml_ng::Value as Yaml;

use super::model::{Group, Model, SectionKind};
use crate::note::Note;

/// The file name of the search data within the site's `assets/`.
pub const SEARCH_DATA: &str = "search-data.js";

/// The search data script for `notes`, which must be the model's notes in
/// the model's order.
pub fn script(model: &Model, notes: &[&Note]) -> String {
    let entries: Vec<serde_json::Value> = notes
        .iter()
        .zip(&model.notes)
        .map(|(note, entry)| {
            serde_json::json!({
                "id": entry.id.to_string(),
                "title": entry.title,
                "page": entry.page,
                "created": entry.created,
                "tags": entry.tags,
                "frontmatter": frontmatter_json(&note.frontmatter),
                "body": note.body,
            })
        })
        .collect();
    let data = serde_json::json!({ "notes": entries, "pages": pages(model) });
    // `</` inside the JSON would end the script element that carries it.
    let json = data.to_string().replace("</", "<\\/");
    format!("window.__ntropySearch={json};\n")
}

/// The section, tag, and group pages, in sidebar order, each with the
/// number of notes it lists.
fn pages(model: &Model) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for section in &model.sections {
        let (kind, field) = match &section.kind {
            SectionKind::Tags => ("tag", None),
            SectionKind::View { field } => ("group", Some(field.as_str())),
        };
        let total: usize = section
            .groups
            .iter()
            .map(|group| group.descendants().len())
            .sum();
        out.push(serde_json::json!({
            "kind": "section",
            "section": section.title,
            "field": field,
            "value": section.title,
            "label": section.title,
            "page": section.page,
            "count": total,
        }));
        fn walk(
            out: &mut Vec<serde_json::Value>,
            groups: &[Group],
            kind: &str,
            section: &str,
            field: Option<&str>,
        ) {
            for group in groups {
                out.push(serde_json::json!({
                    "kind": kind,
                    "section": section,
                    "field": field,
                    "value": group.value,
                    "label": group.label,
                    "page": group.page,
                    "count": group.descendants().len(),
                }));
                walk(out, &group.children, kind, section, field);
            }
        }
        walk(&mut out, &section.groups, kind, &section.title, field);
    }
    out
}

/// A note's frontmatter as the evaluator's JSON.
pub fn frontmatter_json(frontmatter: &serde_yaml_ng::Mapping) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (key, value) in frontmatter {
        if let Yaml::String(key) = key {
            object.insert(key.clone(), value_json(value));
        }
    }
    serde_json::Value::Object(object)
}

fn value_json(value: &Yaml) -> serde_json::Value {
    match value {
        Yaml::Null | Yaml::Tagged(_) => serde_json::Value::Null,
        Yaml::Bool(b) => serde_json::Value::Bool(*b),
        Yaml::Number(n) => serde_json::to_value(n).unwrap_or(serde_json::Value::Null),
        Yaml::String(s) => serde_json::Value::String(s.clone()),
        Yaml::Sequence(items) => serde_json::Value::Array(items.iter().map(value_json).collect()),
        Yaml::Mapping(map) => frontmatter_json(map),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::SiteOptions;
    use crate::text::slug;

    use std::path::PathBuf;

    fn note(id: &str, title: &str, frontmatter: &str, body: &str) -> Note {
        let content = format!("---\ntitle: {title}\n{frontmatter}---\n{body}");
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{id}-{}.md", slug::slugify(title))),
            &content,
            None,
        )
        .expect("the fixture note parses")
    }

    #[test]
    fn frontmatter_keeps_string_keys_and_nulls_tagged_values() {
        let note = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "T",
            "n: 2\nf: 1.5\nb: true\nlist: [a, 1]\nnested:\n  k: v\nangle: !degrees 90\nnone: null\n7: seven\n",
            "",
        );
        let json = frontmatter_json(&note.frontmatter);
        assert_eq!(
            json,
            serde_json::json!({
                "title": "T",
                "n": 2,
                "f": 1.5,
                "b": true,
                "list": ["a", 1],
                "nested": {"k": "v"},
                "angle": null,
                "none": null,
            })
        );
    }

    #[test]
    fn the_script_assigns_the_notes_and_escapes_script_closers() {
        let notes = vec![note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "Danger",
            "tags: [x]\n",
            "Body with </script> inside.\n",
        )];
        let model = Model::build(&notes, &[], &SiteOptions::default(), "V").expect("model");
        let refs: Vec<&Note> = notes.iter().collect();
        let script = script(&model, &refs);
        assert!(
            script.starts_with("window.__ntropySearch={\"notes\":["),
            "{script}"
        );
        assert!(script.ends_with("};\n"), "{script}");
        assert!(!script.contains("</script>"), "{script}");
        assert!(script.contains("<\\/script>"), "{script}");
        let json: serde_json::Value = serde_json::from_str(
            script
                .trim_start_matches("window.__ntropySearch=")
                .trim_end_matches(";\n"),
        )
        .expect("the payload is JSON");
        let first = &json["notes"][0];
        assert_eq!(first["id"], "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(first["title"], "Danger");
        assert_eq!(first["page"], "notes/danger.html");
        assert_eq!(first["tags"], serde_json::json!(["x"]));
        assert_eq!(first["frontmatter"]["title"], "Danger");
        assert!(first["body"].as_str().expect("body").contains("</script>"));
    }

    #[test]
    fn the_pages_list_sections_tags_and_groups_with_counts() {
        let notes = vec![
            note(
                "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "One",
                "tags: [work/rust]\nstatus: open\n",
                "",
            ),
            note("01BRZ3NDEKTSV4RRFFQ69G5FAV", "Two", "tags: [work]\n", ""),
        ];
        let views = [crate::view::ViewDef::new("by-status", "status")];
        let model = Model::build(&notes, &views, &SiteOptions::default(), "V").expect("model");
        let refs: Vec<&Note> = notes.iter().collect();
        let script = script(&model, &refs);
        let json: serde_json::Value = serde_json::from_str(
            script
                .trim_start_matches("window.__ntropySearch=")
                .trim_end_matches(";\n"),
        )
        .expect("the payload is JSON");
        let pages: Vec<(String, String, String, u64)> = json["pages"]
            .as_array()
            .expect("pages")
            .iter()
            .map(|p| {
                (
                    p["kind"].as_str().expect("kind").to_string(),
                    p["value"].as_str().expect("value").to_string(),
                    p["page"].as_str().expect("page").to_string(),
                    p["count"].as_u64().expect("count"),
                )
            })
            .collect();
        assert_eq!(
            pages,
            [
                (
                    "section".into(),
                    "by-status".into(),
                    "views/by-status/index.html".into(),
                    1
                ),
                (
                    "group".into(),
                    "open".into(),
                    "views/by-status/open/index.html".into(),
                    1
                ),
                ("section".into(), "tags".into(), "tags/index.html".into(), 2),
                (
                    "tag".into(),
                    "work".into(),
                    "tags/work/index.html".into(),
                    2
                ),
                (
                    "tag".into(),
                    "work/rust".into(),
                    "tags/work/rust/index.html".into(),
                    1
                ),
            ]
        );
        assert_eq!(json["pages"][1]["field"], "status");
        assert_eq!(json["pages"][3]["field"], serde_json::Value::Null);
    }
}
