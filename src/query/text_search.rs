// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Full-text matching via the embedded ripgrep engine (ADR 0011).
//!
//! A `text:` predicate (and the bare-term shorthand) is a regex matched against
//! a note's in-memory body. The pattern compiles once through `grep-regex` with
//! smart-case enabled (case-insensitive unless the pattern carries an uppercase
//! letter), and matching runs through `grep-searcher`. Compiling up front means
//! an invalid pattern is reported as a query error before any scan begins.

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{Searcher, Sink, SinkMatch};

use super::error::QueryError;

/// A compiled full-text matcher.
#[derive(Debug)]
pub struct TextMatcher {
    matcher: RegexMatcher,
}

impl TextMatcher {
    /// Compile a search pattern, applying smart-case.
    pub fn new(pattern: &str) -> Result<Self, QueryError> {
        let matcher = RegexMatcherBuilder::new()
            .case_smart(true)
            .build(pattern)
            .map_err(|e| QueryError::regex(pattern, e.to_string()))?;
        Ok(Self { matcher })
    }

    /// Whether the body matches the pattern anywhere.
    pub fn is_match(&self, body: &str) -> bool {
        let mut found = false;
        // Searching an in-memory slice cannot fail for any reason we can act on
        // (the sink's only error type is I/O, which a slice never produces), so
        // a search error is treated as "no match".
        let _ = Searcher::new().search_slice(
            &self.matcher,
            body.as_bytes(),
            FoundSink { found: &mut found },
        );
        found
    }
}

/// A sink that records the first match and stops the search immediately.
struct FoundSink<'a> {
    found: &'a mut bool,
}

impl Sink for FoundSink<'_> {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, _mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        *self.found = true;
        // Returning `false` halts the search; one hit is all a predicate needs.
        Ok(false)
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
}
