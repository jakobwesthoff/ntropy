// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The query tokenizer (ADR 0012).
//!
//! A hand-written lexer with no parser dependency. It splits a query string
//! into the few token kinds the grammar needs and records each token's
//! character position so the parser can report positioned errors. Keywords
//! (`and`/`or`/`not`) are lexed as ordinary words and disambiguated later by
//! position, so `field:or` tokenizes the same as any other predicate.

use super::error::QueryError;

/// A lexical token together with its starting character position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// Zero-based character index where the token starts, for error reporting.
    pub pos: usize,
}

/// The kinds of token the grammar distinguishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `:`
    Colon,
    /// A bare word: the `value` production (letters, digits, `/`, `_`, `-`).
    Word(String),
    /// A double-quoted string with its quotes and escapes resolved.
    Str(String),
}

/// Whether `c` may appear in a bare `value` word.
///
/// Unicode alphanumerics are allowed so non-ASCII tag values lex (they are
/// normalized downstream); `/`, `_` and `-` round out the set per the grammar.
/// Anything else (`.`, `+`, `&`, …) is not a word character, so such patterns
/// must be quoted to reach the regex engine.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '/' || c == '_' || c == '-'
}

/// Tokenize a query string.
pub fn tokenize(input: &str) -> Result<Vec<Token>, QueryError> {
    let mut tokens = Vec::new();
    // Character-indexed iteration: positions are reported in characters, which
    // is friendlier than byte offsets for the short strings users type.
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    pos: i,
                });
                i += 1;
            }
            ')' => {
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    pos: i,
                });
                i += 1;
            }
            ':' => {
                tokens.push(Token {
                    kind: TokenKind::Colon,
                    pos: i,
                });
                i += 1;
            }
            '"' => {
                let (value, next) = lex_string(&chars, i)?;
                tokens.push(Token {
                    kind: TokenKind::Str(value),
                    pos: i,
                });
                i = next;
            }
            c if is_word_char(c) => {
                let start = i;
                let mut word = String::new();
                while i < chars.len() && is_word_char(chars[i]) {
                    word.push(chars[i]);
                    i += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Word(word),
                    pos: start,
                });
            }
            other => {
                return Err(QueryError::parse(
                    i,
                    format!("unexpected character `{other}` (quote it to search literally)"),
                ));
            }
        }
    }

    Ok(tokens)
}

/// Lex a double-quoted string starting at `open` (the opening quote).
///
/// Supports `\"` and `\\` escapes so a literal quote or backslash can appear in
/// a phrase. Returns the resolved value and the index just past the closing
/// quote. An unterminated string is a positioned error.
fn lex_string(chars: &[char], open: usize) -> Result<(String, usize), QueryError> {
    let mut value = String::new();
    let mut i = open + 1;
    while i < chars.len() {
        match chars[i] {
            '"' => return Ok((value, i + 1)),
            '\\' if i + 1 < chars.len() => {
                // Only quote and backslash are meaningful escapes; any other
                // escaped char is passed through with its backslash so regex
                // metacharacters in a phrase survive intact.
                match chars[i + 1] {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    other => {
                        value.push('\\');
                        value.push(other);
                    }
                }
                i += 2;
            }
            other => {
                value.push(other);
                i += 1;
            }
        }
    }
    Err(QueryError::parse(
        open,
        "unterminated quoted string".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(input: &str) -> Vec<TokenKind> {
        tokenize(input)
            .expect("tokenize")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lexes_predicate() {
        assert_eq!(
            kinds("tag:work"),
            vec![
                TokenKind::Word("tag".into()),
                TokenKind::Colon,
                TokenKind::Word("work".into()),
            ]
        );
    }

    #[test]
    fn lexes_operators_as_words() {
        assert_eq!(
            kinds("a and b"),
            vec![
                TokenKind::Word("a".into()),
                TokenKind::Word("and".into()),
                TokenKind::Word("b".into()),
            ]
        );
    }

    #[test]
    fn lexes_parens_without_spaces() {
        assert_eq!(
            kinds("(tag:work)"),
            vec![
                TokenKind::LParen,
                TokenKind::Word("tag".into()),
                TokenKind::Colon,
                TokenKind::Word("work".into()),
                TokenKind::RParen,
            ]
        );
    }

    #[test]
    fn lexes_hierarchical_value() {
        assert_eq!(
            kinds("tag:programming/rust"),
            vec![
                TokenKind::Word("tag".into()),
                TokenKind::Colon,
                TokenKind::Word("programming/rust".into()),
            ]
        );
    }

    #[test]
    fn lexes_quoted_string() {
        assert_eq!(
            kinds(r#"text:"in progress""#),
            vec![
                TokenKind::Word("text".into()),
                TokenKind::Colon,
                TokenKind::Str("in progress".into()),
            ]
        );
    }

    #[test]
    fn quoted_string_escapes() {
        assert_eq!(
            kinds(r#""a \"quote\" and \\ slash""#),
            vec![TokenKind::Str(r#"a "quote" and \ slash"#.into())]
        );
    }

    #[test]
    fn quoted_string_preserves_regex_metachars() {
        assert_eq!(
            kinds(r#""foo.*bar""#),
            vec![TokenKind::Str("foo.*bar".into())]
        );
    }

    #[test]
    fn unicode_word_is_lexed() {
        assert_eq!(
            kinds("tag:café"),
            vec![
                TokenKind::Word("tag".into()),
                TokenKind::Colon,
                TokenKind::Word("café".into()),
            ]
        );
    }

    #[test]
    fn unexpected_char_is_positioned_error() {
        let err = tokenize("a + b").expect_err("should fail");
        assert_eq!(err.position(), Some(2));
    }

    #[test]
    fn unterminated_string_is_error() {
        let err = tokenize(r#"text:"unterminated"#).expect_err("should fail");
        assert_eq!(err.position(), Some(5));
    }
}
