// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! View names: the only strings that become view directories (ADR 0064).
//!
//! A view's name is joined onto the vault root, and the view sync deletes
//! every file in that directory that is not one of the view's links. A name
//! is therefore only ever turned into a path as a [`ViewName`], which can be
//! built by [`ViewName::parse`] alone: the type is the proof that the name
//! stays inside the vault, names no hidden directory and no reserved one.
//!
//! A valid name is one or more `/`-separated segments, each already in the
//! normalized slug form that tags and view groups use (ADR 0023), with a
//! first segment that is not one of the reserved top-level names (ADR 0007).

use std::fmt;

use crate::text::slug;
use crate::vault::layout;

/// A validated view name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewName(String);

/// Why a string is not a valid view name.
///
/// The segments these variants carry hold no control character: a name with
/// one is rejected as a whole before any segment is looked at.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidViewName {
    #[error("the name is empty")]
    Empty,
    #[error("the name contains a control character")]
    ControlCharacter,
    #[error("the name starts or ends with `/` or contains `//`")]
    EmptySegment,
    #[error("`{0}` is reserved by ntropy")]
    Reserved(String),
    #[error("the segment `{0}` starts with a dot")]
    DotSegment(String),
    #[error(
        "the segment `{0}` may only hold lowercase letters, digits and single dashes, at most {max} characters",
        max = slug::MAX_SLUG_LEN
    )]
    NotNormalized(String),
}

impl ViewName {
    /// Validate `name` as a view name.
    ///
    /// The checks run in a fixed order so each rejected name gets the most
    /// telling reason: the whole-string rules first, then per segment the
    /// reserved check (so `.ntropy` reads as reserved rather than hidden),
    /// the leading-dot check, and the slug form last.
    pub fn parse(name: &str) -> Result<ViewName, InvalidViewName> {
        if name.is_empty() {
            return Err(InvalidViewName::Empty);
        }
        // Checked on the whole string before any segment message can quote
        // it, and what keeps a name from injecting lines into `.gitignore`.
        if name.chars().any(char::is_control) {
            return Err(InvalidViewName::ControlCharacter);
        }

        for (index, segment) in name.split('/').enumerate() {
            // An empty segment is what a leading `/` (an absolute path), a
            // trailing `/` or a `//` produce.
            if segment.is_empty() {
                return Err(InvalidViewName::EmptySegment);
            }
            // Only the first segment sits at the vault root, where the
            // reserved names live; deeper down they are ordinary names.
            if index == 0 && layout::is_reserved_name(segment) {
                return Err(InvalidViewName::Reserved(segment.to_string()));
            }
            // Covers `.` and `..` as well as hidden directories such as
            // `.git`, which a view sync would otherwise empty.
            if segment.starts_with('.') {
                return Err(InvalidViewName::DotSegment(segment.to_string()));
            }
            // The slug form allows only `[a-z0-9-]`, which also rules out
            // `\`, `:`, uppercase and non-ASCII.
            //
            // TODO(windows): ntropy is Unix only (ADR 0020). A looser rule
            // set would need explicit checks for Windows device names such
            // as `con` or `nul`, which the slug form accepts.
            if slug::normalize_segment(segment) != segment {
                return Err(InvalidViewName::NotNormalized(segment.to_string()));
            }
        }
        Ok(ViewName(name.to_string()))
    }

    /// The nearest valid name to `raw`, if there is one.
    ///
    /// Each segment is normalized the way the slug rule demands and empty
    /// ones are dropped; the result is offered only when it is itself valid,
    /// so a suggestion never leads to a second rejection.
    pub fn suggest(raw: &str) -> Option<ViewName> {
        let segments: Vec<String> = raw
            .split('/')
            .map(slug::normalize_segment)
            .filter(|segment| !segment.is_empty())
            .collect();
        ViewName::parse(&segments.join("/")).ok()
    }

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The `/`-separated segments, outermost first.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl fmt::Display for ViewName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ViewName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(name: &str) -> InvalidViewName {
        ViewName::parse(name).expect_err(&format!("`{}` must be rejected", name.escape_debug()))
    }

    #[test]
    fn accepts_slug_names() {
        for name in ["by-tag", "by-status", "area/work", "2026", "a/b/c"] {
            let parsed = ViewName::parse(name).expect("valid view name");
            assert_eq!(parsed.as_str(), name);
        }
    }

    #[test]
    fn accepts_a_segment_of_the_maximum_length() {
        let name = "a".repeat(slug::MAX_SLUG_LEN);
        assert!(ViewName::parse(&name).is_ok());
    }

    #[test]
    fn rejects_the_empty_name() {
        assert_eq!(rejected(""), InvalidViewName::Empty);
    }

    #[test]
    fn rejects_control_characters() {
        for name in ["by\ntag", "by\0tag", "by\x1btag", "by\ttag", "by\rtag"] {
            assert_eq!(rejected(name), InvalidViewName::ControlCharacter);
        }
    }

    #[test]
    fn rejects_absolute_names_and_empty_segments() {
        for name in ["/etc", "/", "a/", "a//b"] {
            assert_eq!(rejected(name), InvalidViewName::EmptySegment);
        }
    }

    #[test]
    fn rejects_a_reserved_first_segment() {
        for name in [
            "all-notes",
            "all-notes/x",
            ".ntropy",
            ".ntropy/x",
            ".gitignore",
            "README.md",
        ] {
            let first = name.split('/').next().expect("split yields a segment");
            assert_eq!(rejected(name), InvalidViewName::Reserved(first.to_string()));
        }
    }

    #[test]
    fn a_reserved_name_below_the_first_segment_is_an_ordinary_segment() {
        assert!(ViewName::parse("area/all-notes").is_ok());
    }

    #[test]
    fn rejects_dot_segments() {
        for (name, segment) in [
            ("..", ".."),
            ("../x", ".."),
            ("x/..", ".."),
            (".", "."),
            ("a/./b", "."),
            (".git", ".git"),
            ("area/.hidden", ".hidden"),
        ] {
            assert_eq!(
                rejected(name),
                InvalidViewName::DotSegment(segment.to_string())
            );
        }
    }

    #[test]
    fn rejects_segments_outside_the_slug_form() {
        for (name, segment) in [
            ("By_Tag", "By_Tag"),
            ("my.view", "my.view"),
            ("area work", "area work"),
            ("a\\b", "a\\b"),
            ("-x", "-x"),
            ("x-", "x-"),
            ("a--b", "a--b"),
            ("über", "über"),
            ("area/Work", "Work"),
            ("c:", "c:"),
        ] {
            assert_eq!(
                rejected(name),
                InvalidViewName::NotNormalized(segment.to_string())
            );
        }
    }

    #[test]
    fn rejects_a_segment_over_the_maximum_length() {
        let segment = "a".repeat(slug::MAX_SLUG_LEN + 1);
        assert_eq!(
            rejected(&segment),
            InvalidViewName::NotNormalized(segment.clone())
        );
    }

    #[test]
    fn exposes_its_segments() {
        let name = ViewName::parse("area/work").expect("valid view name");
        assert_eq!(name.segments().collect::<Vec<_>>(), ["area", "work"]);
        assert_eq!(name.to_string(), "area/work");
        assert_eq!(name.as_ref(), "area/work");
    }

    #[test]
    fn suggests_the_normalized_name() {
        let suggest = |raw| ViewName::suggest(raw).map(|name| name.to_string());
        assert_eq!(suggest("By_Tag").as_deref(), Some("bytag"));
        assert_eq!(suggest("Area/Work").as_deref(), Some("area/work"));
        assert_eq!(suggest("area work").as_deref(), Some("area-work"));
        assert_eq!(suggest("/etc/").as_deref(), Some("etc"));
    }

    #[test]
    fn suggests_nothing_that_would_be_rejected_again() {
        assert_eq!(ViewName::suggest("All-Notes"), None);
        assert_eq!(ViewName::suggest("!!!"), None);
        assert_eq!(ViewName::suggest(""), None);
    }

    #[test]
    fn messages_name_the_offending_part() {
        let message = |name: &str| rejected(name).to_string();
        assert_eq!(message(""), "the name is empty");
        assert_eq!(message("a\nb"), "the name contains a control character");
        assert_eq!(
            message("/etc"),
            "the name starts or ends with `/` or contains `//`"
        );
        assert_eq!(message("all-notes/x"), "`all-notes` is reserved by ntropy");
        assert_eq!(message("../x"), "the segment `..` starts with a dot");
        assert_eq!(
            message("By_Tag"),
            "the segment `By_Tag` may only hold lowercase letters, digits and single dashes, at most 72 characters"
        );
    }

    #[test]
    fn messages_quote_segments_as_written() {
        // A control character never reaches a segment message (it is
        // rejected first), so the rest is quoted unchanged.
        assert_eq!(
            rejected("it's").to_string(),
            "the segment `it's` may only hold lowercase letters, digits and single dashes, at most 72 characters"
        );
    }
}
