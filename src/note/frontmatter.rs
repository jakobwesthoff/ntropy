// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Frontmatter: delimiter split plus permissive YAML parsing (ADR 0005).
//!
//! A note begins with a `---`-delimited YAML block followed by the Markdown
//! body. The delimiter split is hand-rolled (ADR 0024); the block itself is
//! parsed with `serde_yaml_ng` into a raw mapping so that arbitrary fields are
//! preserved for `field:value` filtering, while the recognized fields `title`
//! (required) and `tags` are lifted out into typed, normalized forms. Numbers
//! and booleans in those two fields are read as their text (ADR 0081); the raw
//! mapping keeps them as the YAML values they parsed to.

use serde_yaml_ng::{Mapping, Value};

use crate::text::tag;

/// The `---` line that opens and closes a frontmatter block.
const FENCE: &str = "---";

/// A note split into its optional frontmatter block and its body.
#[derive(Debug, PartialEq, Eq)]
pub struct Split<'a> {
    /// The text between the fences, if a well-formed block is present.
    pub frontmatter: Option<&'a str>,
    /// Everything after the closing fence (or the whole input when there is no
    /// block).
    pub body: &'a str,
}

/// The parsed frontmatter: recognized fields plus the raw mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Frontmatter {
    /// The canonical title (required, ADR 0005).
    pub title: String,
    /// Tags normalized to their canonical `a/b` segment form (ADR 0023).
    pub tags: Vec<String>,
    /// The full YAML mapping as written, for generic `field:value` matching.
    pub mapping: Mapping,
}

/// Why a frontmatter block is not a usable note header.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    #[error("the note has no frontmatter block")]
    Missing,
    #[error("the frontmatter is not a YAML mapping")]
    NotAMapping,
    #[error("the frontmatter has no `title` field")]
    MissingTitle,
    #[error("the `title` field is {found}, expected a string, number or boolean")]
    InvalidTitle { found: &'static str },
    #[error(
        "entry {position} of the `tags` field is {found}, expected a string, number or boolean"
    )]
    InvalidTag {
        /// The entry's place in the `tags` list, counted from 1.
        position: usize,
        found: &'static str,
    },
    #[error("the `tags` field is a mapping, expected a list or a single value")]
    InvalidTagsField,
    #[error("the frontmatter is not valid YAML")]
    Yaml(#[from] serde_yaml_ng::Error),
}

/// Split a note into its frontmatter block and body without parsing YAML.
///
/// A block is recognized only when the very first line is a `---` fence and a
/// later line is a matching closing `---`. An opening fence with no close is
/// treated as having no frontmatter (the whole input is the body), which lets a
/// later parse step report the note as missing its title rather than silently
/// swallowing content.
pub fn split(content: &str) -> Split<'_> {
    let mut lines = content.lines();
    let first = lines.next();
    if first.map(str::trim_end) != Some(FENCE) {
        return Split {
            frontmatter: None,
            body: content,
        };
    }

    // Find the closing fence and slice the original string by byte offsets so
    // the returned frontmatter and body are borrows, not copies.
    let after_first = first.expect("checked above").len();
    // Skip the newline following the opening fence.
    let mut offset = after_first;
    offset += newline_len(&content[offset..]);
    let block_start = offset;

    for line in content[block_start..].split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.trim_end() == FENCE {
            let block_end = offset;
            let body_start = offset + line.len();
            return Split {
                frontmatter: Some(&content[block_start..block_end]),
                body: &content[body_start..],
            };
        }
        offset += line.len();
    }

    // Opening fence never closed.
    Split {
        frontmatter: None,
        body: content,
    }
}

/// Length in bytes of a leading newline (`\n` or `\r\n`), or 0 if none.
fn newline_len(s: &str) -> usize {
    if s.starts_with("\r\n") {
        2
    } else if s.starts_with('\n') {
        1
    } else {
        0
    }
}

/// Parse a frontmatter block into recognized fields plus the raw mapping.
pub fn parse_block(block: &str) -> Result<Frontmatter, FrontmatterError> {
    let value: Value = serde_yaml_ng::from_str(block)?;

    let mapping = match value {
        Value::Mapping(m) => m,
        // An empty block deserializes to null; treat it as an empty mapping so
        // the only complaint is the missing title.
        Value::Null => Mapping::new(),
        _ => return Err(FrontmatterError::NotAMapping),
    };

    let title = match mapping.get(Value::from("title")).map(classify) {
        Some(Scalar::Text(text)) if !text.trim().is_empty() => text,
        Some(Scalar::Collection(found)) => return Err(FrontmatterError::InvalidTitle { found }),
        _ => return Err(FrontmatterError::MissingTitle),
    };

    let tags = extract_tags(&mapping)?;

    Ok(Frontmatter {
        title,
        tags,
        mapping,
    })
}

/// What a frontmatter value is, as far as the recognized fields care.
enum Scalar {
    /// A string, number or boolean, as the text a user would read.
    Text(String),
    /// `null`: the field is there but holds nothing.
    Absent,
    /// A list or a mapping, named for the error message.
    Collection(&'static str),
}

/// The value under any YAML tag (`!label foo`), which `Value::as_str` has
/// always looked through, so tagged titles and tags keep reading as their
/// scalar.
fn untag(mut value: &Value) -> &Value {
    while let Value::Tagged(tagged) = value {
        value = &tagged.value;
    }
    value
}

/// A string, number or boolean as the text it is read as, `None` for every
/// other kind of value.
///
/// The YAML parser keeps no source spelling, so a number reads as the number
/// it parsed to: `1.50` is `1.5` and `0x1F` is `31`. A value whose exact
/// spelling matters has to be quoted.
pub(crate) fn scalar_text(value: &Value) -> Option<String> {
    match untag(value) {
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Sort a value of a recognized field into text, nothing or a collection.
fn classify(value: &Value) -> Scalar {
    if let Some(text) = scalar_text(value) {
        return Scalar::Text(text);
    }
    match untag(value) {
        Value::Sequence(_) => Scalar::Collection("a list"),
        Value::Mapping(_) => Scalar::Collection("a mapping"),
        _ => Scalar::Absent,
    }
}

/// Pull the `tags` field out as a normalized, de-duplicated segment list.
///
/// Accepts either a YAML sequence (the documented form) or a lone scalar (a
/// lenient convenience), reads each entry as text, normalizes it and drops
/// any that normalize to nothing. Order is preserved; exact duplicates are
/// collapsed. A `null` entry is skipped. A list or mapping entry is refused,
/// naming its position, since dropping it would hide a tag the user wrote.
fn extract_tags(mapping: &Mapping) -> Result<Vec<String>, FrontmatterError> {
    let entries: Vec<&Value> = match mapping.get(Value::from("tags")).map(untag) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Sequence(seq)) => seq.iter().collect(),
        Some(Value::Mapping(_)) => return Err(FrontmatterError::InvalidTagsField),
        Some(lone) => vec![lone],
    };

    let mut seen = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let text = match classify(entry) {
            Scalar::Text(text) => text,
            Scalar::Absent => continue,
            Scalar::Collection(found) => {
                return Err(FrontmatterError::InvalidTag {
                    position: index + 1,
                    found,
                });
            }
        };
        let normalized = tag::normalize(&text);
        if !normalized.is_empty() && !seen.contains(&normalized) {
            seen.push(normalized);
        }
    }
    Ok(seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_extracts_block_and_body() {
        let content = "---\ntitle: Hello\n---\n# Body\ntext\n";
        let split = split(content);
        assert_eq!(split.frontmatter, Some("title: Hello\n"));
        assert_eq!(split.body, "# Body\ntext\n");
    }

    #[test]
    fn split_handles_crlf() {
        let content = "---\r\ntitle: Hello\r\n---\r\nBody\r\n";
        let split = split(content);
        assert_eq!(split.frontmatter, Some("title: Hello\r\n"));
        assert_eq!(split.body, "Body\r\n");
    }

    #[test]
    fn split_without_opening_fence_is_all_body() {
        let content = "# Just a heading\nno frontmatter\n";
        let split = split(content);
        assert_eq!(split.frontmatter, None);
        assert_eq!(split.body, content);
    }

    #[test]
    fn split_unclosed_fence_is_all_body() {
        let content = "---\ntitle: Hello\nnever closes\n";
        let split = split(content);
        assert_eq!(split.frontmatter, None);
        assert_eq!(split.body, content);
    }

    #[test]
    fn split_empty_block() {
        let content = "---\n---\nbody\n";
        let split = split(content);
        assert_eq!(split.frontmatter, Some(""));
        assert_eq!(split.body, "body\n");
    }

    #[test]
    fn parse_extracts_title_and_tags() {
        let fm =
            parse_block("title: My Note\ntags: [Programming/Rust, area/work]\n").expect("parse");
        assert_eq!(fm.title, "My Note");
        assert_eq!(fm.tags, vec!["programming/rust", "area/work"]);
    }

    #[test]
    fn parse_preserves_arbitrary_fields() {
        let fm = parse_block("title: T\nstatus: in progress\npriority: 3\n").expect("parse");
        assert_eq!(
            fm.mapping
                .get(Value::from("status"))
                .and_then(Value::as_str),
            Some("in progress")
        );
        assert_eq!(
            fm.mapping
                .get(Value::from("priority"))
                .and_then(Value::as_i64),
            Some(3)
        );
    }

    #[test]
    fn parse_accepts_scalar_tag() {
        let fm = parse_block("title: T\ntags: Work\n").expect("parse");
        assert_eq!(fm.tags, vec!["work"]);
    }

    #[test]
    fn parse_dedups_and_drops_empty_tags() {
        let fm = parse_block("title: T\ntags: [Rust, rust, \"///\"]\n").expect("parse");
        assert_eq!(fm.tags, vec!["rust"]);
    }

    /// The title of a block that must parse, for the coercion tests.
    fn title_of(block: &str) -> String {
        parse_block(block)
            .unwrap_or_else(|e| panic!("{block:?} should parse: {e}"))
            .title
    }

    /// The tags of a block that must parse, for the coercion tests.
    fn tags_of(block: &str) -> Vec<String> {
        parse_block(block)
            .unwrap_or_else(|e| panic!("{block:?} should parse: {e}"))
            .tags
    }

    /// The message of a block that must be refused.
    fn refusal_of(block: &str) -> String {
        match parse_block(block) {
            Ok(fm) => panic!("{block:?} should be refused, got {fm:?}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn integer_title_is_its_text() {
        assert_eq!(title_of("title: 2026\n"), "2026");
    }

    #[test]
    fn float_title_is_its_text() {
        assert_eq!(title_of("title: 3.14\n"), "3.14");
    }

    #[test]
    fn float_title_loses_a_trailing_zero_the_parser_drops() {
        assert_eq!(title_of("title: 1.50\n"), "1.5");
    }

    #[test]
    fn version_like_float_title_reads_as_the_parsed_number() {
        assert_eq!(title_of("title: 1.10\n"), "1.1");
    }

    #[test]
    fn integral_float_title_keeps_its_fraction() {
        assert_eq!(title_of("title: 1.0\n"), "1.0");
    }

    #[test]
    fn hexadecimal_title_is_the_decimal_number() {
        assert_eq!(title_of("title: 0x1F\n"), "31");
    }

    #[test]
    fn boolean_title_is_true_or_false() {
        assert_eq!(title_of("title: True\n"), "true");
    }

    #[test]
    fn non_finite_title_keeps_the_yaml_spelling() {
        assert_eq!(title_of("title: .inf\n"), ".inf");
    }

    #[test]
    fn tagged_title_reads_as_its_scalar() {
        assert_eq!(title_of("title: !note Hello\n"), "Hello");
    }

    #[test]
    fn null_title_is_missing() {
        assert!(matches!(
            parse_block("title: null\n"),
            Err(FrontmatterError::MissingTitle)
        ));
    }

    #[test]
    fn list_title_says_it_is_a_list() {
        assert_eq!(
            refusal_of("title: [a, b]\n"),
            "the `title` field is a list, expected a string, number or boolean"
        );
    }

    #[test]
    fn mapping_title_says_it_is_a_mapping() {
        assert_eq!(
            refusal_of("title: {a: b}\n"),
            "the `title` field is a mapping, expected a string, number or boolean"
        );
    }

    #[test]
    fn numeric_tags_are_kept() {
        assert_eq!(
            tags_of("title: T\ntags: [2026, rust]\n"),
            vec!["2026", "rust"]
        );
    }

    #[test]
    fn float_and_boolean_tags_are_kept() {
        assert_eq!(
            tags_of("title: T\ntags: [1.50, 1.0, true]\n"),
            vec![
                tag::normalize("1.5"),
                tag::normalize("1.0"),
                "true".to_string()
            ]
        );
    }

    #[test]
    fn lone_number_tag_is_one_tag() {
        assert_eq!(tags_of("title: T\ntags: 2026\n"), vec!["2026"]);
    }

    #[test]
    fn lone_tagged_tag_reads_as_its_scalar() {
        assert_eq!(tags_of("title: T\ntags: !label foo\n"), vec!["foo"]);
    }

    #[test]
    fn tagged_tag_list_reads_as_the_list() {
        assert_eq!(tags_of("title: T\ntags: !label [a, b]\n"), vec!["a", "b"]);
    }

    #[test]
    fn equal_numbers_collapse_into_one_tag() {
        assert_eq!(tags_of("title: T\ntags: [1.50, 1.5]\n").len(), 1);
    }

    #[test]
    fn null_tag_entries_are_skipped() {
        assert_eq!(tags_of("title: T\ntags: [null, a]\n"), vec!["a"]);
    }

    #[test]
    fn list_tag_entry_names_its_position() {
        assert_eq!(
            refusal_of("title: T\ntags: [a, [b, c]]\n"),
            "entry 2 of the `tags` field is a list, expected a string, number or boolean"
        );
    }

    #[test]
    fn mapping_tag_entry_names_its_position() {
        assert_eq!(
            refusal_of("title: T\ntags: [{a: b}]\n"),
            "entry 1 of the `tags` field is a mapping, expected a string, number or boolean"
        );
    }

    #[test]
    fn mapping_tags_field_is_refused() {
        assert_eq!(
            refusal_of("title: T\ntags: {a: b}\n"),
            "the `tags` field is a mapping, expected a list or a single value"
        );
    }

    #[test]
    fn parse_missing_title_is_error() {
        assert!(matches!(
            parse_block("tags: [a]\n"),
            Err(FrontmatterError::MissingTitle)
        ));
    }

    #[test]
    fn parse_blank_title_is_error() {
        assert!(matches!(
            parse_block("title: \"   \"\n"),
            Err(FrontmatterError::MissingTitle)
        ));
    }

    #[test]
    fn parse_empty_block_is_missing_title() {
        assert!(matches!(
            parse_block(""),
            Err(FrontmatterError::MissingTitle)
        ));
    }

    #[test]
    fn parse_non_mapping_is_error() {
        assert!(matches!(
            parse_block("- just\n- a\n- list\n"),
            Err(FrontmatterError::NotAMapping)
        ));
    }

    #[test]
    fn parse_malformed_yaml_is_error() {
        assert!(matches!(
            parse_block("title: [unterminated\n"),
            Err(FrontmatterError::Yaml(_))
        ));
    }
}
