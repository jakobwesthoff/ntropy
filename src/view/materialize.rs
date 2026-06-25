// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Materializing one view as a symlink tree (ADRs 0008, 0009, 0023).
//!
//! A view groups notes by one frontmatter field. Each grouping value (always
//! normalized) becomes a directory path under the view, a `/` in the value
//! nests further, and a list-valued field places a note under each of its
//! values. The leaf in each group is a relative symlink back into `all-notes/`,
//! named `<date>-<slug>.md` with collisions disambiguated (see [`super::leaf`]).
//! A note with no value for the field is skipped.

use std::collections::BTreeMap;

use serde_yaml_ng::Value;

use crate::error::Result;
use crate::fsutil;
use crate::note::Note;
use crate::text::{slug, tag};
use crate::vault::Vault;

use super::ViewDef;
use super::leaf::{self, LeafInput};

/// Rebuild a single view's directory from scratch.
///
/// The directory is wiped and recreated first, so the regenerated tree is
/// exactly the current projection and any stale or orphaned links are pruned
/// (ADR 0008).
pub fn build_view(vault: &Vault, view: &ViewDef, notes: &[Note]) -> Result<()> {
    let view_dir = vault.layout().view_dir(&view.name);
    fsutil::wipe_and_recreate_dir(&view_dir)?;

    // Group notes by normalized field value. A `BTreeMap` keeps the build
    // deterministic, which matters for reproducible link creation and tests.
    let mut groups: BTreeMap<String, Vec<&Note>> = BTreeMap::new();
    for note in notes {
        for value in group_values(note, &view.field) {
            groups.entry(value).or_default().push(note);
        }
    }

    for (value, group_notes) in groups {
        // A value's `/` segments nest into subdirectories.
        let leaf_dir = view_dir.join(&value);

        let mut inputs = Vec::with_capacity(group_notes.len());
        for note in &group_notes {
            inputs.push(LeafInput {
                id: note.id,
                date: note.created_date()?,
                slug: slug::slugify(&note.title),
            });
        }
        let names = leaf::leaf_names(&inputs);

        for (note, name) in group_notes.iter().zip(names) {
            let link = leaf_dir.join(&name);
            // The stored target is relative to the link's own directory, so the
            // vault stays relocatable (ADR 0008).
            let target = fsutil::relative_path(&leaf_dir, &note.path);
            fsutil::symlink(&target, &link)?;
        }
    }

    Ok(())
}

/// The normalized grouping values a note contributes for `field`.
///
/// `tags` is taken from the already-normalized tag list; any other field is
/// read from the raw frontmatter, accepting a scalar or a sequence and
/// normalizing each value the same way tags are (ADR 0009). Values that
/// normalize to nothing, and missing/non-scalar fields, contribute nothing, so
/// such notes are skipped.
fn group_values(note: &Note, field: &str) -> Vec<String> {
    if field == "tags" {
        return note.tags.clone();
    }

    let raw: Vec<String> = match note.frontmatter.get(Value::from(field)) {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(scalar_to_string).collect(),
        Some(scalar) => scalar_to_string(scalar).into_iter().collect(),
        None => Vec::new(),
    };

    let mut values = Vec::new();
    for entry in raw {
        let normalized = tag::normalize(&entry);
        if !normalized.is_empty() && !values.contains(&normalized) {
            values.push(normalized);
        }
    }
    values
}

/// Render a scalar YAML value to its string form, or `None` for non-scalars.
fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn note(id: &str, frontmatter: &str) -> Note {
        let content = format!("---\n{frontmatter}---\nbody\n");
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{id}-slug.md")),
            &content,
            None,
        )
        .expect("parse note")
    }

    #[test]
    fn tags_field_uses_normalized_tags() {
        let n = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "title: T\ntags: [Programming/Rust, area/work]\n",
        );
        assert_eq!(
            group_values(&n, "tags"),
            vec!["programming/rust", "area/work"]
        );
    }

    #[test]
    fn arbitrary_field_is_normalized() {
        let n = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "title: T\nstatus: In Progress\n",
        );
        assert_eq!(group_values(&n, "status"), vec!["in-progress"]);
    }

    #[test]
    fn numeric_field_groups_by_its_text() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\npriority: 2\n");
        assert_eq!(group_values(&n, "priority"), vec!["2"]);
    }

    #[test]
    fn list_field_fans_out() {
        let n = note(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "title: T\nareas: [Home, Work]\n",
        );
        assert_eq!(group_values(&n, "areas"), vec!["home", "work"]);
    }

    #[test]
    fn missing_field_yields_no_groups() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\n");
        assert!(group_values(&n, "status").is_empty());
    }

    #[test]
    fn unnormalizable_value_is_dropped() {
        let n = note("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title: T\nstatus: \"!!!\"\n");
        assert!(group_values(&n, "status").is_empty());
    }
}
