// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tag normalization and the sub-path match predicate (ADRs 0006, 0023).
//!
//! A tag is a `/`-separated hierarchy. Each segment is normalized with the slug
//! rules, the `/` separators are preserved, and empty segments are dropped so a
//! tag canonicalizes to a clean lowercase form (`Rust` and `rust` are the same
//! tag). Matching a query tag against a note tag is a contiguous-segment-run
//! search, which is the semantics the `tag:` query predicate evaluates.

use super::slug::normalize_segment;

/// Split a tag into its normalized, non-empty segments.
///
/// Each `/`-delimited piece is run through the slug segment normalizer; pieces
/// that normalize to nothing (e.g. from `a//b` or a stray separator) are
/// dropped. This is the canonical decomposition every other tag operation is
/// built on.
pub fn segments(tag: &str) -> Vec<String> {
    tag.split('/')
        .map(normalize_segment)
        .filter(|seg| !seg.is_empty())
        .collect()
}

/// Normalize a tag to its canonical `a/b/c` string form.
///
/// Returns an empty string when the tag has no surviving segments, which the
/// note model treats as "no tag".
pub fn normalize(tag: &str) -> String {
    segments(tag).join("/")
}

/// Test whether `query` matches `candidate` under the sub-path rule (ADR 0006
/// as refined for v1).
///
/// Both sides are decomposed into normalized segments; the query matches iff
/// its segment list occurs as a contiguous run of full segments anywhere in the
/// candidate's segment list. So `programming` matches `programming`,
/// `programming/foo`, `bar/programming` and `baz/programming/blub`, while
/// `programming/foo` matches any tag containing that consecutive chain.
/// Normalization makes the match case-insensitive. An empty query matches
/// nothing.
pub fn matches(query: &str, candidate: &str) -> bool {
    let needle = segments(query);
    if needle.is_empty() {
        return false;
    }
    let haystack = segments(candidate);
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Existing tags that could complete the partial tag being typed.
///
/// Matching is hierarchy-aware. The partial's leading segments must equal the
/// candidate's, and the candidate's segment at the partial's depth must start
/// with the partial's last segment, so `prog` offers `programming/rust` and
/// `programming/ru` narrows to `programming/rust`. A partial ending in `/`
/// offers the strictly deeper children below it. An empty partial offers every
/// candidate. The partial is normalized, so matching is case-insensitive.
/// Order is preserved and duplicates are dropped.
pub fn suggest(partial: &str, candidates: &[String]) -> Vec<String> {
    let needle = segments(partial);
    let trailing_slash = partial.ends_with('/');

    let mut seen = std::collections::HashSet::new();
    candidates
        .iter()
        .filter(|candidate| {
            if needle.is_empty() {
                return true;
            }
            let haystack = segments(candidate);
            if trailing_slash {
                // Offer children strictly below the completed partial.
                haystack.len() > needle.len() && haystack[..needle.len()] == needle[..]
            } else {
                let (last, parents) = needle.split_last().expect("needle is non-empty");
                haystack.len() >= needle.len()
                    && haystack[..parents.len()] == parents[..]
                    && haystack[parents.len()].starts_with(last)
            }
        })
        .filter(|candidate| seen.insert((*candidate).clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn suggest_empty_partial_offers_all() {
        let all = tags(&["programming/rust", "area/work"]);
        assert_eq!(suggest("", &all), all);
    }

    #[test]
    fn suggest_prefixes_the_first_segment() {
        let all = tags(&["programming/rust", "programming/cli", "area/work"]);
        assert_eq!(
            suggest("prog", &all),
            tags(&["programming/rust", "programming/cli"])
        );
    }

    #[test]
    fn suggest_trailing_slash_offers_children() {
        let all = tags(&["programming", "programming/rust", "programming/cli"]);
        assert_eq!(
            suggest("programming/", &all),
            tags(&["programming/rust", "programming/cli"])
        );
    }

    #[test]
    fn suggest_narrows_within_a_level() {
        let all = tags(&["programming/rust", "programming/cli"]);
        assert_eq!(suggest("programming/ru", &all), tags(&["programming/rust"]));
    }

    #[test]
    fn suggest_is_case_insensitive_and_normalizing() {
        let all = tags(&["ueber/groesse"]);
        assert_eq!(suggest("Über", &all), tags(&["ueber/groesse"]));
    }

    #[test]
    fn suggest_drops_duplicates_and_non_matches() {
        let all = tags(&["area/work", "area/work", "programming/rust"]);
        assert_eq!(suggest("area", &all), tags(&["area/work"]));
        assert!(suggest("xyz", &all).is_empty());
    }

    #[test]
    fn segments_normalize_each_piece() {
        assert_eq!(segments("Programming/Rust"), vec!["programming", "rust"]);
        assert_eq!(segments("Area/Work"), vec!["area", "work"]);
    }

    #[test]
    fn segments_drop_empty_pieces() {
        assert_eq!(segments("a//b"), vec!["a", "b"]);
        assert_eq!(segments("/leading/"), vec!["leading"]);
        assert_eq!(segments(""), Vec::<String>::new());
    }

    #[test]
    fn normalize_roundtrips_to_canonical_string() {
        assert_eq!(normalize("Programming/Rust"), "programming/rust");
        assert_eq!(normalize("Über/Größe"), "ueber/groesse");
        assert_eq!(normalize("///"), "");
    }

    #[test]
    fn matches_single_segment_anywhere() {
        assert!(matches("programming", "programming"));
        assert!(matches("programming", "programming/foo"));
        assert!(matches("programming", "bar/programming"));
        assert!(matches("programming", "baz/programming/blub"));
    }

    #[test]
    fn matches_multi_segment_contiguous_run() {
        assert!(matches("programming/foo", "programming/foo"));
        assert!(matches("programming/foo", "x/programming/foo/y"));
        // Not contiguous: a segment intervenes.
        assert!(!matches("programming/foo", "programming/bar/foo"));
    }

    #[test]
    fn matches_is_case_insensitive() {
        assert!(matches("Rust", "programming/rust"));
        assert!(matches("rust", "Programming/Rust"));
    }

    #[test]
    fn matches_requires_full_segment_not_substring() {
        // `prog` is not a segment of `programming`.
        assert!(!matches("prog", "programming"));
    }

    #[test]
    fn empty_query_matches_nothing() {
        assert!(!matches("", "anything"));
        assert!(!matches("///", "anything"));
    }
}
