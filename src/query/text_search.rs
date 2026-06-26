// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Full-text matching via the `regex` crate (ADR 0030).
//!
//! A `text:` predicate (and the bare-term shorthand) is a regex matched against
//! a note's in-memory body. Because the body is already parsed and resident, a
//! predicate is a plain [`Regex::is_match`] over a string slice; the file-scale
//! machinery of the ripgrep stack offered nothing here (ADR 0030). The pattern
//! compiles once, so an invalid pattern is reported as a query error before any
//! scan begins.
//!
//! Two behaviors are carried over from the previous matcher:
//!
//! - **Smart case**: matching is case-insensitive unless the pattern carries a
//!   *literal* uppercase character (see [`smart_case_insensitive`]).
//! - **Line anchors**: `multi_line` is enabled so `^`/`$` bind to line
//!   boundaries and `.` does not cross a newline, preserving the line-oriented
//!   feel of grep for the predicates the query DSL can build.

use regex::{Regex, RegexBuilder};
use regex_syntax::ast::{Ast, ClassSet, ClassSetItem, parse::Parser};

use super::error::QueryError;

/// A compiled full-text matcher.
#[derive(Debug)]
pub struct TextMatcher {
    regex: Regex,
}

impl TextMatcher {
    /// Compile a search pattern, applying smart-case.
    pub fn new(pattern: &str) -> Result<Self, QueryError> {
        let regex = RegexBuilder::new(pattern)
            .case_insensitive(smart_case_insensitive(pattern))
            .multi_line(true)
            .build()
            .map_err(|e| QueryError::regex(pattern, e.to_string()))?;
        Ok(Self { regex })
    }

    /// Whether the body matches the pattern anywhere.
    pub fn is_match(&self, body: &str) -> bool {
        self.regex.is_match(body)
    }
}

// =============================================================================
// Smart case
// =============================================================================

/// Whether a smart-case search of `pattern` should be case-insensitive.
///
/// The rule mirrors ripgrep's: insensitive when the pattern contains at least
/// one literal character and none of its literals is uppercase. Crucially, only
/// *literals* count. A class shorthand like `\W` or `\pL` carries an uppercase
/// letter in its syntax but contributes no literal, so `foo\W` still searches
/// insensitively while `Foo` does not.
///
/// A pattern that does not parse is treated as case-sensitive; the real
/// compilation error then surfaces from [`RegexBuilder::build`].
fn smart_case_insensitive(pattern: &str) -> bool {
    match Parser::new().parse(pattern) {
        Ok(ast) => {
            let mut analysis = LiteralCase::default();
            analysis.visit(&ast);
            analysis.any_literal && !analysis.any_uppercase
        }
        Err(_) => false,
    }
}

/// Accumulates, over a regex AST, whether any literal exists and whether any of
/// those literals is uppercase. Class shorthands and assertions are deliberately
/// ignored so only genuine literal characters influence smart case.
#[derive(Default)]
struct LiteralCase {
    any_literal: bool,
    any_uppercase: bool,
}

impl LiteralCase {
    /// Walk an AST node, folding every literal it reaches into the analysis.
    fn visit(&mut self, ast: &Ast) {
        match ast {
            Ast::Literal(lit) => self.record(lit.c),
            Ast::ClassBracketed(class) => self.visit_class_set(&class.kind),
            Ast::Repetition(rep) => self.visit(&rep.ast),
            Ast::Group(group) => self.visit(&group.ast),
            Ast::Alternation(alt) => alt.asts.iter().for_each(|a| self.visit(a)),
            Ast::Concat(concat) => concat.asts.iter().for_each(|a| self.visit(a)),
            // Empty, flags, `.`, assertions, and Unicode/Perl classes hold no
            // literal character, so they cannot affect smart case.
            _ => {}
        }
    }

    /// Walk a bracketed class, where literals appear directly or as the bounds
    /// of a range (so `[A-Z]` registers an uppercase literal).
    fn visit_class_set(&mut self, set: &ClassSet) {
        match set {
            ClassSet::Item(item) => self.visit_class_item(item),
            ClassSet::BinaryOp(op) => {
                self.visit_class_set(&op.lhs);
                self.visit_class_set(&op.rhs);
            }
        }
    }

    fn visit_class_item(&mut self, item: &ClassSetItem) {
        match item {
            ClassSetItem::Literal(lit) => self.record(lit.c),
            ClassSetItem::Range(range) => {
                self.record(range.start.c);
                self.record(range.end.c);
            }
            ClassSetItem::Bracketed(class) => self.visit_class_set(&class.kind),
            ClassSetItem::Union(union) => {
                union.items.iter().for_each(|i| self.visit_class_item(i))
            }
            // Empty, ASCII/Unicode/Perl class shorthands carry no literal.
            _ => {}
        }
    }

    /// Fold one literal character into the running analysis.
    fn record(&mut self, c: char) {
        self.any_literal = true;
        self.any_uppercase = self.any_uppercase || c.is_uppercase();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_term_matches_substring() {
        let m = TextMatcher::new("deadline").expect("compile");
        assert!(m.is_match("the deadline is near"));
        assert!(!m.is_match("nothing here"));
    }

    #[test]
    fn smart_case_lowercase_is_insensitive() {
        let m = TextMatcher::new("deadline").expect("compile");
        assert!(m.is_match("DEADLINE in caps"));
        assert!(m.is_match("Deadline mixed"));
    }

    #[test]
    fn smart_case_uppercase_is_sensitive() {
        let m = TextMatcher::new("Deadline").expect("compile");
        assert!(m.is_match("Deadline mixed"));
        assert!(!m.is_match("deadline lower"));
    }

    #[test]
    fn regex_metacharacters_work() {
        let m = TextMatcher::new("foo.*bar").expect("compile");
        assert!(m.is_match("fooXXXbar"));
        assert!(!m.is_match("bar foo"));
    }

    #[test]
    fn matches_across_multiple_lines() {
        let m = TextMatcher::new("second").expect("compile");
        assert!(m.is_match("first line\nsecond line\n"));
    }

    #[test]
    fn invalid_regex_is_query_error() {
        let err = TextMatcher::new("[unterminated").expect_err("should fail");
        assert!(matches!(err, QueryError::Regex { .. }));
    }

    // =====================================================================
    // Smart-case literal analysis
    // =====================================================================

    #[test]
    fn smart_case_class_shorthand_does_not_force_sensitivity() {
        // `\W` carries an uppercase letter in its syntax but no literal, so the
        // all-lowercase literal `foo` keeps the search case-insensitive.
        let m = TextMatcher::new(r"foo\W").expect("compile");
        assert!(m.is_match("FOO!"));
    }

    #[test]
    fn smart_case_unicode_class_has_no_literal() {
        // `\pL` is purely a class: no literal at all means smart case leaves the
        // search sensitive (the absence of literals never flips to insensitive).
        assert!(!smart_case_insensitive(r"\pL"));
    }

    #[test]
    fn smart_case_uppercase_in_range_is_sensitive() {
        // A bracketed range bound is a literal, so `[A-Z]` forces sensitivity.
        assert!(!smart_case_insensitive(r"foo[A-Z]"));
    }

    #[test]
    fn smart_case_lowercase_range_stays_insensitive() {
        assert!(smart_case_insensitive(r"foo[a-z]"));
    }

    #[test]
    fn smart_case_empty_pattern_has_no_literal() {
        assert!(!smart_case_insensitive(""));
    }

    #[test]
    fn smart_case_uppercase_inside_group_is_sensitive() {
        assert!(!smart_case_insensitive(r"(Foo|bar)"));
    }

    #[test]
    fn smart_case_lowercase_alternation_is_insensitive() {
        assert!(smart_case_insensitive(r"(foo|bar)"));
    }

    // =====================================================================
    // Line-anchor semantics (multi_line)
    // =====================================================================

    #[test]
    fn caret_anchors_to_each_line_start() {
        // multi_line binds `^` to every line start, not just the buffer start.
        let m = TextMatcher::new("^second").expect("compile");
        assert!(m.is_match("first line\nsecond line\n"));
        assert!(!m.is_match("a first\nthe second\n"));
    }

    #[test]
    fn dot_does_not_cross_newline() {
        // `.` stays within a line, matching the prior line-oriented behavior.
        let m = TextMatcher::new("foo.bar").expect("compile");
        assert!(!m.is_match("foo\nbar"));
        assert!(m.is_match("fooXbar"));
    }
}
