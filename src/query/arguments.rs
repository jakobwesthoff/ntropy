// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Turning the command line's trailing arguments into one query (ADR 0076).
//!
//! A shell hands over `ntropy search "borrow checker" tag:work` as two
//! arguments, and the quotes are gone. Each argument is therefore read on its
//! own: one that holds several words and no query syntax is a phrase, and one
//! that is a query stays a query. The result is a single string for the query
//! parser, which treats operands side by side as an `and`.

use regex::escape;

use super::token::{Token, TokenKind, tokenize};

/// The words that act as operators when they stand in operator position.
const OPERATORS: [&str; 3] = ["and", "or", "not"];

/// Build the query string for the arguments a command received.
///
/// Blank arguments are dropped and the rest trimmed, so an empty selector
/// comes out as an empty string.
pub fn from_arguments(arguments: &[String]) -> String {
    let arguments: Vec<&str> = arguments
        .iter()
        .map(|argument| argument.trim())
        .filter(|argument| !argument.is_empty())
        .collect();

    // A lone argument keeps its text as typed, so a syntax error in it is
    // reported at the position the user sees. Beside others, a whole query is
    // grouped, which keeps `search 'a or b' c` from becoming `a or (b and c)`.
    let grouped = arguments.len() > 1;
    arguments
        .iter()
        .map(|argument| read(argument, grouped))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Turn one non-blank argument into its part of the query.
fn read(argument: &str, grouped: bool) -> String {
    // The lexer splits tokens at `char::is_whitespace`, so this does too.
    if !argument.chars().any(char::is_whitespace) {
        return argument.to_string();
    }
    if !has_query_syntax(argument) {
        return phrase(argument);
    }
    if grouped && is_complete_query(argument) {
        format!("({argument})")
    } else {
        argument.to_string()
    }
}

/// Whether the argument contains anything the query language gives a meaning.
fn has_query_syntax(argument: &str) -> bool {
    argument.contains([':', '(', ')', '"'])
        || argument
            .split_whitespace()
            .any(|word| is_operator_word(word, &OPERATORS))
}

/// Whether `word` is spelled like one of `operators`; the parser compares
/// keywords with `eq_ignore_ascii_case`, so this does too.
fn is_operator_word(word: &str, operators: &[&str]) -> bool {
    operators.iter().any(|op| word.eq_ignore_ascii_case(op))
}

/// The query for a phrase: its words in order, with any whitespace (a line
/// break included) between them, every other character taken literally.
fn phrase(argument: &str) -> String {
    let pattern = argument
        .split_whitespace()
        .map(escape)
        .collect::<Vec<_>>()
        .join(r"\s+");
    // The lexer reads `\\` inside a quoted string as one backslash, so each
    // backslash of the pattern is doubled to arrive unchanged.
    format!("\"{}\"", pattern.replace('\\', r"\\"))
}

/// Whether the argument is a query that stands on its own, as opposed to a
/// piece of one that was split at a space.
///
/// A piece is joined to its neighbours without parentheses, as a plain join
/// would, so `search tag:work 'and not status:done'` keeps working.
fn is_complete_query(argument: &str) -> bool {
    let Ok(tokens) = tokenize(argument) else {
        return false;
    };

    let mut depth = 0usize;
    for token in &tokens {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => match depth.checked_sub(1) {
                Some(open) => depth = open,
                None => return false,
            },
            _ => {}
        }
    }
    if depth != 0 {
        return false;
    }

    // `and` or `or` opens a piece unless a `:` follows, which makes it a field.
    let followed_by_colon = tokens
        .get(1)
        .is_some_and(|second| second.kind == TokenKind::Colon);
    let opens_with_operator = tokens
        .first()
        .is_some_and(|first| is_operator_token(first, &["and", "or"]))
        && !followed_by_colon;
    let closes_with_operator = tokens
        .last()
        .is_some_and(|last| is_operator_token(last, &OPERATORS));
    !opens_with_operator && !closes_with_operator
}

/// Whether the token is a word spelled like one of `operators`.
fn is_operator_token(token: &Token, operators: &[&str]) -> bool {
    match &token.kind {
        TokenKind::Word(word) => is_operator_word(word, operators),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::note::Note;
    use crate::query::compile;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    fn query(items: &[&str]) -> String {
        from_arguments(&args(items))
    }

    fn note(body: &str) -> Note {
        let content = format!("---\ntitle: T\n---\n{body}");
        Note::parse(
            PathBuf::from("/v/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-t.md"),
            &content,
            None,
        )
        .expect("parse note")
    }

    /// Whether the query built from `items` matches a note with this body.
    fn finds(items: &[&str], body: &str) -> bool {
        compile(&query(items))
            .expect("the generated query compiles")
            .matches(&note(body))
    }

    #[test]
    fn one_word_is_unchanged() {
        assert_eq!(query(&["borrow"]), "borrow");
    }

    #[test]
    fn a_typed_term_is_unchanged() {
        assert_eq!(query(&["tag:work"]), "tag:work");
    }

    #[test]
    fn an_operator_argument_is_unchanged() {
        assert_eq!(query(&["or"]), "or");
    }

    #[test]
    fn a_spaced_argument_is_a_phrase() {
        assert_eq!(query(&["borrow checker"]), r#""borrow\\s+checker""#);
    }

    #[test]
    fn a_tab_separates_the_words_of_a_phrase() {
        assert_eq!(query(&["a\tb"]), r#""a\\s+b""#);
    }

    #[test]
    fn a_phrase_matches_its_words_in_order() {
        assert!(finds(&["borrow checker"], "the borrow checker complains"));
        assert!(!finds(&["borrow checker"], "checker, then borrow"));
    }

    #[test]
    fn a_phrase_matches_across_a_line_break() {
        assert!(finds(&["borrow checker"], "the borrow\nchecker complains"));
    }

    #[test]
    fn a_phrase_matches_across_a_run_of_spaces() {
        assert!(finds(&["borrow checker"], "the borrow   checker complains"));
    }

    #[test]
    fn a_phrase_matches_regex_metacharacters_literally() {
        assert!(finds(&["c++ tips"], "learn c++ tips"));
        assert!(!finds(&["c++ tips"], "learn ccc tips"));
        assert!(finds(&["node.js tips"], "node.js tips"));
        assert!(!finds(&["node.js tips"], "nodexjs tips"));
    }

    #[test]
    fn a_phrase_matches_a_backslash_literally() {
        assert!(finds(&[r"a\b c"], r"see a\b c here"));
        assert!(!finds(&[r"a\b c"], "see ab c here"));
    }

    #[test]
    fn a_phrase_is_case_insensitive_unless_it_has_an_uppercase_letter() {
        assert!(finds(&["borrow checker"], "Borrow Checker"));
        assert!(!finds(&["Borrow checker"], "borrow checker"));
        assert!(finds(&["Borrow checker"], "Borrow checker"));
    }

    #[test]
    fn a_whole_query_in_one_argument_is_unchanged() {
        assert_eq!(
            query(&["tag:work and not status:done"]),
            "tag:work and not status:done"
        );
        assert_eq!(query(&["(a or b)"]), "(a or b)");
    }

    #[test]
    fn a_word_that_is_an_operator_in_any_case_makes_the_argument_a_query() {
        assert_eq!(query(&["a OR b"]), "a OR b");
        assert_eq!(query(&["x And y"]), "x And y");
        assert_eq!(query(&["not z w"]), "not z w");
        assert_eq!(query(&["rock and roll"]), "rock and roll");
    }

    #[test]
    fn a_word_that_only_contains_an_operator_is_still_a_phrase() {
        assert_eq!(query(&["Android notes"]), r#""Android\\s+notes""#);
    }

    #[test]
    fn a_colon_a_parenthesis_or_a_quote_makes_the_argument_a_query() {
        assert_eq!(query(&["meeting at 10:30"]), "meeting at 10:30");
        assert_eq!(query(&["page (draft)"]), "page (draft)");
        assert_eq!(query(&[r#"say "hello" now"#]), r#"say "hello" now"#);
    }

    #[test]
    fn a_phrase_with_query_syntax_is_read_as_a_query() {
        assert_eq!(query(&["page not found"]), "page not found");
    }

    #[test]
    fn a_quoted_phrase_inside_an_argument_is_unchanged() {
        assert_eq!(query(&[r#""page not found""#]), r#""page not found""#);
    }

    #[test]
    fn arguments_are_joined_by_a_space() {
        assert_eq!(query(&["tag:work", "tag:home"]), "tag:work tag:home");
    }

    #[test]
    fn a_phrase_argument_joins_the_other_arguments() {
        assert_eq!(
            query(&["borrow checker", "tag:work"]),
            r#""borrow\\s+checker" tag:work"#
        );
    }

    #[test]
    fn operators_given_as_arguments_of_their_own_keep_working() {
        assert_eq!(
            query(&["tag:work", "or", "tag:home"]),
            "tag:work or tag:home"
        );
        assert_eq!(
            query(&["borrow checker", "or", "rock roll"]),
            r#""borrow\\s+checker" or "rock\\s+roll""#
        );
    }

    #[test]
    fn a_whole_query_argument_is_grouped_beside_other_arguments() {
        assert_eq!(query(&["a or b", "c"]), "(a or b) c");
    }

    #[test]
    fn grouping_keeps_the_precedence_of_a_query_argument() {
        assert_eq!(
            query(&["tag:work", "and", "status:open or status:new"]),
            "tag:work and (status:open or status:new)"
        );
    }

    #[test]
    fn a_parenthesised_query_argument_is_grouped_beside_other_arguments() {
        assert_eq!(query(&["(a or b) c", "d"]), "((a or b) c) d");
    }

    #[test]
    fn a_quoted_phrase_argument_is_grouped_beside_other_arguments() {
        assert_eq!(
            query(&[r#""page not found""#, "x"]),
            r#"("page not found") x"#
        );
    }

    #[test]
    fn a_field_named_like_an_operator_does_not_make_a_fragment() {
        assert_eq!(query(&["or:x or y", "z"]), "(or:x or y) z");
    }

    #[test]
    fn a_single_token_argument_is_never_grouped() {
        assert_eq!(
            query(&["tag:work", "or", "tag:home"]),
            "tag:work or tag:home"
        );
        assert_eq!(query(&["not", "tag:a"]), "not tag:a");
    }

    #[test]
    fn a_query_argument_that_starts_with_an_operator_is_a_fragment() {
        assert_eq!(
            query(&["tag:work", "and not status:done"]),
            "tag:work and not status:done"
        );
        assert_eq!(query(&["x", "or tag:y"]), "x or tag:y");
    }

    #[test]
    fn a_query_argument_that_ends_with_an_operator_is_a_fragment() {
        assert_eq!(query(&["tag:a or", "tag:b"]), "tag:a or tag:b");
        assert_eq!(query(&["x and", "y"]), "x and y");
        assert_eq!(query(&["tag:a and not", "tag:b"]), "tag:a and not tag:b");
    }

    #[test]
    fn a_query_argument_with_unbalanced_parentheses_is_a_fragment() {
        assert_eq!(query(&["(tag:a or", "tag:b)"]), "(tag:a or tag:b)");
        assert_eq!(query(&["x", "tag:a or tag:b)"]), "x tag:a or tag:b)");
    }

    #[test]
    fn a_query_argument_the_tokenizer_rejects_is_a_fragment() {
        assert_eq!(query(&[r#"a "b c"#, "d"]), r#"a "b c d"#);
    }

    #[test]
    fn blank_arguments_are_dropped() {
        assert_eq!(query(&["", "tag:work", "  "]), "tag:work");
        assert_eq!(query(&[""]), "");
        assert_eq!(query(&[]), "");
    }

    #[test]
    fn a_dropped_blank_argument_does_not_count_for_grouping() {
        assert_eq!(query(&["a or b", ""]), "a or b");
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(query(&["  tag:work  "]), "tag:work");
        assert_eq!(query(&[" borrow checker "]), r#""borrow\\s+checker""#);
    }

    #[test]
    fn a_lone_ulid_stays_a_ulid() {
        assert_eq!(
            query(&["01ARZ3NDEKTSV4RRFFQ69G5FAV"]),
            "01ARZ3NDEKTSV4RRFFQ69G5FAV"
        );
    }
}
