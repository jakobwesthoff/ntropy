// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Frontmatter: delimiter split plus permissive YAML parsing (ADR 0005).
//!
//! A note begins with a `---`-delimited YAML block followed by the Markdown
//! body. The delimiter split is hand-rolled (ADR 0024); the block itself is
//! parsed with `serde_yaml_ng` into a raw mapping so that arbitrary fields are
//! preserved for `field:value` filtering, while the recognized fields `title`
//! (required) and `tags` are lifted out into typed, normalized forms.

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

    let title = mapping
        .get(Value::from("title"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|t| !t.trim().is_empty())
        .ok_or(FrontmatterError::MissingTitle)?;

    let tags = extract_tags(&mapping);

    Ok(Frontmatter {
        title,
        tags,
        mapping,
    })
}

/// Pull the `tags` field out as a normalized, de-duplicated segment list.
///
/// Accepts either a YAML sequence (the documented form) or a lone scalar (a
/// lenient convenience), normalizing each entry and dropping any that
/// normalize to nothing. Order is preserved; exact duplicates are collapsed.
fn extract_tags(mapping: &Mapping) -> Vec<String> {
    let raw = match mapping.get(Value::from("tags")) {
        Some(Value::Sequence(seq)) => seq
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    };

    let mut seen = Vec::new();
    for entry in raw {
        let normalized = tag::normalize(&entry);
        if !normalized.is_empty() && !seen.contains(&normalized) {
            seen.push(normalized);
        }
    }
    seen
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
