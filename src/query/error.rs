// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The query concern's error type (ADR 0012).
//!
//! Both kinds of failure a query can have are surfaced here: a positioned
//! syntax error from the tokenizer/parser, and an invalid `text:` regex. The
//! plan calls for the regex failure to read like a parse error, so they share
//! one type and one human-facing shape.

/// A failure parsing or preparing a query.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueryError {
    /// A syntax error at a known character position in the query string.
    #[error("query syntax error at position {pos}: {message}")]
    Parse { pos: usize, message: String },

    /// A `text:`/bare-term pattern was not a valid regex.
    #[error("invalid search pattern `{pattern}`: {message}")]
    Regex { pattern: String, message: String },
}

impl QueryError {
    /// Construct a positioned parse error.
    pub fn parse(pos: usize, message: impl Into<String>) -> Self {
        QueryError::Parse {
            pos,
            message: message.into(),
        }
    }

    /// Construct a regex-compilation error.
    pub fn regex(pattern: impl Into<String>, message: impl Into<String>) -> Self {
        QueryError::Regex {
            pattern: pattern.into(),
            message: message.into(),
        }
    }

    /// The character position of a parse error, if this is one.
    pub fn position(&self) -> Option<usize> {
        match self {
            QueryError::Parse { pos, .. } => Some(*pos),
            QueryError::Regex { .. } => None,
        }
    }
}
