// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Recursive-descent parser for the query grammar (ADR 0012).
//!
//! Precedence is `not > and > or`, with parentheses overriding. Two operands
//! side by side are an `and` (ADR 0076). Keywords are
//! disambiguated by position: a `Word` only acts as an operator when it sits in
//! operator position *and* is not immediately followed by a `:` (so `field:or`
//! parses as a predicate). The v1 predicate grammar is:
//!
//! ```text
//! predicate := "tag" ":" (value | string)
//!            | "text" ":" (value | string)
//!            | field ":" (value | string)
//!            | value | string            -- bare term: shorthand for text:
//! ```

use super::ast::Query;
use super::error::QueryError;
use super::token::{Token, TokenKind, tokenize};

/// Parse a query string into an AST.
pub fn parse(input: &str) -> Result<Query, QueryError> {
    let tokens = tokenize(input)?;
    let eof_pos = input.chars().count();
    let mut parser = Parser {
        tokens,
        idx: 0,
        eof_pos,
    };

    if parser.tokens.is_empty() {
        return Err(QueryError::parse(0, "expected a query"));
    }

    let query = parser.parse_or()?;

    // Anything left over means the expression ended early (e.g. a dangling
    // operator or an unbalanced `)`).
    if let Some(tok) = parser.peek() {
        return Err(QueryError::parse(
            tok.pos,
            "unexpected trailing input".to_string(),
        ));
    }

    Ok(query)
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
    eof_pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.idx)
    }

    fn peek_kind_at(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens.get(self.idx + offset).map(|t| &t.kind)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.idx).cloned();
        if tok.is_some() {
            self.idx += 1;
        }
        tok
    }

    /// Whether the current token is the operator keyword `kw`.
    ///
    /// A keyword in operator position only counts as an operator when it is not
    /// immediately followed by `:`; `or:foo` is the `or` *field* predicate, not
    /// the boolean operator.
    fn at_operator(&self, kw: &str) -> bool {
        match self.peek_kind_at(0) {
            Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(kw) => {
                !matches!(self.peek_kind_at(1), Some(TokenKind::Colon))
            }
            _ => false,
        }
    }

    // or := and ("or" and)*
    fn parse_or(&mut self) -> Result<Query, QueryError> {
        let mut left = self.parse_and()?;
        while self.at_operator("or") {
            self.advance();
            let right = self.parse_and()?;
            left = Query::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    // and := unary (["and"] unary)*
    //
    // Two operands side by side are an `and` as well, so `search borrow checker`
    // (the shell hands over its words joined by spaces) finds notes holding
    // both words. Because the loop lives here, below `or`, the implicit `and`
    // binds exactly like the written one.
    fn parse_and(&mut self) -> Result<Query, QueryError> {
        let mut left = self.parse_unary()?;
        loop {
            if self.at_operator("and") {
                self.advance();
            } else if !self.starts_operand() {
                break;
            }
            let right = self.parse_unary()?;
            left = Query::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// Whether the next token begins another operand of an `and` chain.
    ///
    /// The operator `or`, a `)`, a stray `:` and the end of input end the
    /// chain; the caller reports the last two as "unexpected trailing input".
    /// Every other word starts an operand, including `not` and an `or` that a
    /// `:` follows (the field `or`).
    fn starts_operand(&self) -> bool {
        match self.peek_kind_at(0) {
            Some(TokenKind::LParen | TokenKind::Str(_)) => true,
            Some(TokenKind::Word(_)) => !self.at_operator("or"),
            Some(TokenKind::RParen | TokenKind::Colon) | None => false,
        }
    }

    // unary := "not" unary | primary
    fn parse_unary(&mut self) -> Result<Query, QueryError> {
        if self.at_operator("not") {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Query::Not(Box::new(operand)));
        }
        self.parse_primary()
    }

    // primary := "(" or ")" | predicate
    fn parse_primary(&mut self) -> Result<Query, QueryError> {
        if matches!(self.peek_kind_at(0), Some(TokenKind::LParen)) {
            self.advance();
            let inner = self.parse_or()?;
            match self.advance() {
                Some(Token {
                    kind: TokenKind::RParen,
                    ..
                }) => Ok(inner),
                Some(tok) => Err(QueryError::parse(tok.pos, "expected `)`".to_string())),
                None => Err(QueryError::parse(self.eof_pos, "expected `)`".to_string())),
            }
        } else {
            self.parse_predicate()
        }
    }

    fn parse_predicate(&mut self) -> Result<Query, QueryError> {
        let tok = self
            .advance()
            .ok_or_else(|| QueryError::parse(self.eof_pos, "expected a predicate"))?;

        let word = match tok.kind {
            // A bare quoted phrase is a full-text shorthand.
            TokenKind::Str(s) => return Ok(Query::Text(s)),
            TokenKind::Word(w) => w,
            other => {
                return Err(QueryError::parse(
                    tok.pos,
                    format!("expected a predicate, found `{}`", render_kind(&other)),
                ));
            }
        };

        // A `:` makes this a keyed predicate; otherwise the bare word is
        // full-text shorthand.
        if !matches!(self.peek_kind_at(0), Some(TokenKind::Colon)) {
            return Ok(Query::Text(word));
        }
        self.advance(); // consume the colon

        let value = match self.advance() {
            Some(Token {
                kind: TokenKind::Word(w),
                ..
            }) => w,
            Some(Token {
                kind: TokenKind::Str(s),
                ..
            }) => s,
            Some(tok) => {
                return Err(QueryError::parse(
                    tok.pos,
                    "expected a value after `:`".to_string(),
                ));
            }
            None => {
                return Err(QueryError::parse(
                    self.eof_pos,
                    "expected a value after `:`".to_string(),
                ));
            }
        };

        // `tag` and `text` are recognized field keys (case-insensitively); any
        // other key is a generic frontmatter field predicate.
        if word.eq_ignore_ascii_case("tag") {
            Ok(Query::Tag(value))
        } else if word.eq_ignore_ascii_case("text") {
            Ok(Query::Text(value))
        } else {
            Ok(Query::Field { name: word, value })
        }
    }
}

/// A short human-readable rendering of a token kind for error messages.
fn render_kind(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::LParen => "(",
        TokenKind::RParen => ")",
        TokenKind::Colon => ":",
        TokenKind::Word(_) => "word",
        TokenKind::Str(_) => "string",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> Query {
        parse(input).expect("parse")
    }

    #[test]
    fn single_tag_predicate() {
        insta::assert_debug_snapshot!(parse_ok("tag:work"), @r#"
        Tag(
            "work",
        )
        "#);
    }

    #[test]
    fn bare_word_is_text() {
        assert_eq!(parse_ok("deadline"), Query::Text("deadline".into()));
    }

    #[test]
    fn bare_quoted_is_text() {
        assert_eq!(
            parse_ok(r#""in progress""#),
            Query::Text("in progress".into())
        );
    }

    #[test]
    fn field_predicate_with_quoted_value() {
        assert_eq!(
            parse_ok(r#"status:"in progress""#),
            Query::Field {
                name: "status".into(),
                value: "in progress".into()
            }
        );
    }

    #[test]
    fn precedence_not_over_and_over_or() {
        // not a and b or c  ==  ((not a) and b) or c
        insta::assert_debug_snapshot!(parse_ok("not a and b or c"), @r#"
        Or(
            And(
                Not(
                    Text(
                        "a",
                    ),
                ),
                Text(
                    "b",
                ),
            ),
            Text(
                "c",
            ),
        )
        "#);
    }

    #[test]
    fn parentheses_override_precedence() {
        insta::assert_debug_snapshot!(parse_ok("a and (b or c)"), @r#"
        And(
            Text(
                "a",
            ),
            Or(
                Text(
                    "b",
                ),
                Text(
                    "c",
                ),
            ),
        )
        "#);
    }

    #[test]
    fn keyword_as_field_value_by_position() {
        // `field:or` is a predicate, not the boolean operator.
        assert_eq!(
            parse_ok("status:or"),
            Query::Field {
                name: "status".into(),
                value: "or".into()
            }
        );
    }

    #[test]
    fn combined_text_and_tag() {
        insta::assert_debug_snapshot!(parse_ok("foobar and tag:work"), @r#"
        And(
            Text(
                "foobar",
            ),
            Tag(
                "work",
            ),
        )
        "#);
    }

    #[test]
    fn tag_keyword_is_case_insensitive() {
        assert_eq!(parse_ok("Tag:Work"), Query::Tag("Work".into()));
    }

    #[test]
    fn empty_query_is_error() {
        let err = parse("").expect_err("empty");
        assert_eq!(err.position(), Some(0));
    }

    #[test]
    fn dangling_operator_is_positioned_error() {
        let err = parse("tag:work and").expect_err("dangling");
        // The `and` is consumed, then a unary is expected at end-of-input.
        assert_eq!(err.position(), Some("tag:work and".chars().count()));
    }

    #[test]
    fn unbalanced_paren_is_error() {
        let err = parse("(tag:work").expect_err("unbalanced");
        assert_eq!(err.position(), Some("(tag:work".chars().count()));
    }

    #[test]
    fn missing_value_after_colon_is_error() {
        let err = parse("tag:").expect_err("missing value");
        assert_eq!(err.position(), Some(4));
    }

    fn text(word: &str) -> Query {
        Query::Text(word.into())
    }

    fn and(left: Query, right: Query) -> Query {
        Query::And(Box::new(left), Box::new(right))
    }

    #[test]
    fn adjacent_bare_words_are_an_implicit_and() {
        assert_eq!(
            parse_ok("borrow checker"),
            and(text("borrow"), text("checker"))
        );
    }

    #[test]
    fn adjacent_text_and_tag_are_an_implicit_and() {
        assert_eq!(
            parse_ok("watcher tag:settings"),
            and(text("watcher"), Query::Tag("settings".into()))
        );
    }

    #[test]
    fn implicit_and_chains_left_to_right() {
        assert_eq!(parse_ok("a b c"), and(and(text("a"), text("b")), text("c")));
    }

    #[test]
    fn implicit_and_binds_tighter_than_or() {
        // a b or c  ==  (a and b) or c
        assert_eq!(
            parse_ok("a b or c"),
            Query::Or(Box::new(and(text("a"), text("b"))), Box::new(text("c")))
        );
    }

    #[test]
    fn implicit_and_follows_a_group() {
        assert_eq!(parse_ok("(a) b"), and(text("a"), text("b")));
    }

    #[test]
    fn implicit_and_joins_a_quoted_phrase() {
        assert_eq!(
            parse_ok(r#"tag:programming "borrow checker""#),
            and(Query::Tag("programming".into()), text("borrow checker"))
        );
    }

    #[test]
    fn not_applies_to_the_next_term_only() {
        // not a b  ==  (not a) and b
        assert_eq!(
            parse_ok("not a b"),
            and(Query::Not(Box::new(text("a"))), text("b"))
        );
    }

    #[test]
    fn not_after_a_term_starts_a_negated_operand() {
        assert_eq!(
            parse_ok("a not b"),
            and(text("a"), Query::Not(Box::new(text("b"))))
        );
    }

    #[test]
    fn keyword_field_predicate_joins_as_a_term() {
        // `or:x` is a field predicate, so it must not end the `and` chain.
        assert_eq!(
            parse_ok("a or:x"),
            and(
                text("a"),
                Query::Field {
                    name: "or".into(),
                    value: "x".into()
                }
            )
        );
    }

    #[test]
    fn leading_keyword_is_a_search_word() {
        assert_eq!(
            parse_ok("and tag:x"),
            and(text("and"), Query::Tag("x".into()))
        );
    }

    #[test]
    fn doubled_keyword_searches_for_the_keyword() {
        assert_eq!(
            parse_ok("a and and b"),
            and(and(text("a"), text("and")), text("b"))
        );
    }

    #[test]
    fn closing_paren_after_a_chain_is_trailing_input() {
        let err = parse("a b )").expect_err("trailing");
        assert_eq!(
            err.to_string(),
            "query syntax error at position 4: unexpected trailing input"
        );
    }

    #[test]
    fn colon_after_a_group_is_trailing_input() {
        let err = parse("(a) :").expect_err("trailing");
        assert_eq!(
            err.to_string(),
            "query syntax error at position 4: unexpected trailing input"
        );
    }

    #[test]
    fn colon_after_a_predicate_is_trailing_input() {
        let err = parse("tag:x :").expect_err("trailing");
        assert_eq!(
            err.to_string(),
            "query syntax error at position 6: unexpected trailing input"
        );
    }

    #[test]
    fn dangling_operator_after_an_implicit_and_is_an_error() {
        let err = parse("a b and").expect_err("dangling");
        assert_eq!(err.position(), Some("a b and".chars().count()));
    }

    #[test]
    fn unclosed_group_around_adjacent_terms_reports_the_end() {
        let err = parse("(a b").expect_err("unbalanced");
        assert_eq!(err.position(), Some(4));
    }

    #[test]
    fn a_dotted_tag_value_needs_no_quotes() {
        assert_eq!(
            parse_ok("tag:milestone/0.13"),
            Query::Tag("milestone/0.13".into())
        );
    }

    #[test]
    fn a_dotted_field_value_needs_no_quotes() {
        assert_eq!(
            parse_ok("version:1.5"),
            Query::Field {
                name: "version".into(),
                value: "1.5".into()
            }
        );
    }
}
