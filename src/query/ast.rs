// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The query abstract syntax tree (ADR 0012).
//!
//! The parser produces this tree; it is a faithful, regex-free representation
//! of the query so it can be snapshot-tested directly. Compiling `text` nodes
//! into searchers happens in a separate preparation step (see
//! [`super::eval`]), keeping the AST cheap to build and inspect.

/// A parsed query expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// `a and b`
    And(Box<Query>, Box<Query>),
    /// `a or b`
    Or(Box<Query>, Box<Query>),
    /// `not a`
    Not(Box<Query>),
    /// `tag:value` — segment sub-path match (ADRs 0006, 0023).
    Tag(String),
    /// `field:value` — frontmatter equality or list membership.
    Field { name: String, value: String },
    /// `text:"…"`, or a bare term/phrase — a regex over the body (ADR 0030).
    Text(String),
}
