// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Query evaluation against a parsed note (ADRs 0002, 0006, 0011).
//!
//! The AST is first lowered into a [`Prepared`] query: structurally identical,
//! but every `text` node carries a compiled regex matcher, so an invalid
//! pattern fails here, once, before any scanning. Evaluation is then a pure
//! single-pass walk over a note: tag predicates use the segment sub-path rule,
//! field predicates compare frontmatter values exactly, and text predicates run
//! the embedded grep engine over the in-memory body.

use serde_yaml_ng::Value;

use crate::note::Note;
use crate::text::tag;

use super::ast::Query;
use super::error::QueryError;
use super::text_search::TextMatcher;

/// A query with its text patterns compiled, ready to match notes.
pub enum Prepared {
    And(Box<Prepared>, Box<Prepared>),
    Or(Box<Prepared>, Box<Prepared>),
    Not(Box<Prepared>),
    Tag(String),
    Field { name: String, value: String },
    Text(TextMatcher),
}

impl Prepared {
    /// Lower an AST into a prepared query, compiling all text patterns.
    pub fn from_ast(query: &Query) -> Result<Prepared, QueryError> {
        Ok(match query {
            Query::And(a, b) => Prepared::And(
                Box::new(Prepared::from_ast(a)?),
                Box::new(Prepared::from_ast(b)?),
            ),
            Query::Or(a, b) => Prepared::Or(
                Box::new(Prepared::from_ast(a)?),
                Box::new(Prepared::from_ast(b)?),
            ),
            Query::Not(a) => Prepared::Not(Box::new(Prepared::from_ast(a)?)),
            Query::Tag(t) => Prepared::Tag(t.clone()),
            Query::Field { name, value } => Prepared::Field {
                name: name.clone(),
                value: value.clone(),
            },
            Query::Text(pattern) => Prepared::Text(TextMatcher::new(pattern)?),
        })
    }

    /// Whether `note` satisfies the query.
    pub fn matches(&self, note: &Note) -> bool {
        match self {
            Prepared::And(a, b) => a.matches(note) && b.matches(note),
            Prepared::Or(a, b) => a.matches(note) || b.matches(note),
            Prepared::Not(a) => !a.matches(note),
            Prepared::Tag(query) => note.tags.iter().any(|t| tag::matches(query, t)),
            Prepared::Field { name, value } => field_matches(note, name, value),
            Prepared::Text(matcher) => matcher.is_match(&note.body),
        }
    }
}

/// Exact frontmatter match: scalar equality, or membership for a list field.
fn field_matches(note: &Note, name: &str, value: &str) -> bool {
    match note.frontmatter.get(Value::from(name)) {
        Some(Value::Sequence(seq)) => seq
            .iter()
            .any(|item| scalar_to_string(item).as_deref() == Some(value)),
        Some(scalar) => scalar_to_string(scalar).as_deref() == Some(value),
        None => false,
    }
}

/// Render a scalar YAML value to the string a `field:value` predicate compares
/// against. Non-scalars (nested maps/sequences) have no scalar form and never
/// match.
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
    use crate::query::parser::parse;
    use std::path::PathBuf;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn note(frontmatter: &str, body: &str) -> Note {
        let content = format!("---\n{frontmatter}---\n{body}");
        Note::parse(
            PathBuf::from(format!("/v/all-notes/{ULID}-n.md")),
            &content,
            None,
        )
        .expect("parse note")
    }

    fn matches(query: &str, note: &Note) -> bool {
        Prepared::from_ast(&parse(query).expect("parse query"))
            .expect("prepare")
            .matches(note)
    }

    #[test]
    fn tag_subpath_matches() {
        let n = note("title: T\ntags: [programming/rust, area/work]\n", "");
        assert!(matches("tag:programming", &n));
        assert!(matches("tag:rust", &n));
        assert!(matches("tag:programming/rust", &n));
        assert!(matches("tag:work", &n));
        assert!(!matches("tag:leisure", &n));
    }

    #[test]
    fn tag_match_is_case_insensitive() {
        let n = note("title: T\ntags: [Programming/Rust]\n", "");
        assert!(matches("tag:rust", &n));
        assert!(matches("Tag:PROGRAMMING", &n));
    }

    #[test]
    fn field_scalar_equality() {
        let n = note("title: T\nstatus: done\n", "");
        assert!(matches("status:done", &n));
        assert!(!matches("status:open", &n));
    }

    #[test]
    fn field_quoted_multiword_value() {
        let n = note("title: T\nstatus: in progress\n", "");
        assert!(matches(r#"status:"in progress""#, &n));
        assert!(!matches("status:progress", &n));
    }

    #[test]
    fn field_numeric_and_bool() {
        let n = note("title: T\npriority: 3\npinned: true\n", "");
        assert!(matches("priority:3", &n));
        assert!(matches("pinned:true", &n));
        assert!(!matches("priority:5", &n));
    }

    #[test]
    fn field_list_membership() {
        let n = note("title: T\nareas: [home, work]\n", "");
        assert!(matches("areas:home", &n));
        assert!(matches("areas:work", &n));
        assert!(!matches("areas:school", &n));
    }

    #[test]
    fn missing_field_does_not_match() {
        let n = note("title: T\n", "");
        assert!(!matches("status:done", &n));
    }

    #[test]
    fn text_searches_body_with_smart_case() {
        let n = note("title: T\n", "The DEADLINE is friday.\n");
        assert!(matches("deadline", &n));
        assert!(matches(r#"text:"friday""#, &n));
        assert!(!matches("Deadline", &n)); // uppercase → case-sensitive, body has DEADLINE
        assert!(!matches("monday", &n));
    }

    #[test]
    fn boolean_composition() {
        let n = note(
            "title: T\ntags: [area/work]\nstatus: open\n",
            "deadline soon",
        );
        assert!(matches("tag:area and status:open", &n));
        assert!(matches("tag:area and not status:done", &n));
        assert!(!matches("tag:area and status:done", &n));
        assert!(matches("status:done or deadline", &n));
        assert!(matches("(tag:area or tag:home) and deadline", &n));
    }

    #[test]
    fn invalid_regex_surfaces_on_prepare() {
        let ast = parse(r#""[unterminated""#).expect("parse");
        assert!(matches!(
            Prepared::from_ast(&ast),
            Err(QueryError::Regex { .. })
        ));
    }
}
