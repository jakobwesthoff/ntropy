// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Shared `nucleo` fuzzy ranking for the language server (ADR 0027).
//!
//! Several LSP features rank a set of entries against a typed query identically:
//! an empty query keeps input order, while a non-empty one fuzzy-matches each
//! entry's haystack and sorts by score (descending), breaking ties by input
//! index so equal-score entries keep their original (newest-first) order. The
//! only thing that varies between call sites is the haystack a given entry
//! contributes, so that is the single parameter.

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

/// Rank `entries` against `query`, returning references in result order.
///
/// `haystack` extracts the matchable text for one entry. An empty query short
/// circuits to every entry in input order, doing no matching work. Each call
/// builds a fresh [`Matcher`]; callers that re-rank on every keystroke (the
/// picker) keep their own persistent matcher instead.
pub fn rank<'e, E>(query: &str, entries: &'e [E], haystack: impl Fn(&E) -> String) -> Vec<&'e E> {
    if query.is_empty() {
        return entries.iter().collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut scratch = Vec::new();
    let mut scored: Vec<(u32, usize, &E)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let haystack = Utf32String::from(haystack(entry));
            pattern
                .indices(haystack.slice(..), &mut matcher, &mut scratch)
                .map(|score| {
                    scratch.clear();
                    (score, index, entry)
                })
        })
        .collect();
    // Best score first; equal scores keep their original input order.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn empty_query_keeps_every_entry_in_input_order() {
        let items = items(&["gamma", "beta", "alpha"]);
        let ranked = rank("", &items, |s: &String| s.clone());
        assert_eq!(ranked, vec![&items[0], &items[1], &items[2]]);
    }

    #[test]
    fn ranks_matches_and_drops_non_matches() {
        let items = items(&["alpha", "beta", "alabaster"]);
        let ranked = rank("alpha", &items, |s: &String| s.clone());
        // "beta" has no `alpha` subsequence, so it is dropped; "alpha" wins.
        assert!(!ranked.iter().any(|s| s.as_str() == "beta"));
        assert_eq!(ranked[0].as_str(), "alpha");
    }

    #[test]
    fn equal_scores_break_ties_by_input_index() {
        // Identical haystacks score equally, so input order is preserved.
        let items = items(&["first", "second", "third"]);
        let ranked = rank("match", &items, |_| "match".to_owned());
        assert_eq!(ranked[0].as_str(), "first");
        assert_eq!(ranked[1].as_str(), "second");
        assert_eq!(ranked[2].as_str(), "third");
    }
}
