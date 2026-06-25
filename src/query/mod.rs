// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The query DSL: the single filtering and full-text mechanism (ADR 0012).
//!
//! A query string is tokenized, parsed into an AST ([`Query`]), and then
//! compiled into a [`Prepared`] query whose text patterns are ready regexes.
//! `parse` is exposed separately so the AST can be inspected and snapshot
//! tested; [`compile`] is the one-call path the use-case layer uses to go from
//! a string straight to something it can match notes against. Both surface a
//! single [`QueryError`].

mod ast;
mod error;
mod eval;
mod parser;
mod text_search;
mod token;

pub use ast::Query;
pub use error::QueryError;
pub use eval::Prepared;

/// Parse a query string into its AST without compiling regexes.
pub fn parse(input: &str) -> Result<Query, QueryError> {
    parser::parse(input)
}

/// Parse and compile a query string into a ready-to-match [`Prepared`] query.
pub fn compile(input: &str) -> Result<Prepared, QueryError> {
    Prepared::from_ast(&parser::parse(input)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_reports_parse_errors() {
        assert!(matches!(compile("tag:"), Err(QueryError::Parse { .. })));
    }

    #[test]
    fn compile_reports_regex_errors() {
        assert!(matches!(
            compile(r#""[bad""#),
            Err(QueryError::Regex { .. })
        ));
    }
}
