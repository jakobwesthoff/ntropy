// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Column alignment for picker rows (ADR 0027).
//!
//! The picker shows every candidate as an aligned grid: `title │ date │ tags`
//! followed by the display-only ULID. Aligning the columns needs the widths of
//! *all* candidates at once (the title column is padded to the widest title,
//! and so on), so this is a batch step run before the picker starts rather than
//! a per-row render. The matchable string carries the padding verbatim, which
//! keeps the fuzzy match positions aligned with what is drawn.
//!
//! Column widths are derived from absolute caps, not the terminal width, so the
//! grid is stable across a resize; only the final per-line truncation in the
//! draw loop reacts to width.

use ntropy::ops::Candidate;

use super::Row;

/// Max display width (in chars) of the title column before ellipsis truncation.
const TITLE_CAP: usize = 48;
/// Max display width (in chars) of the bracketed tags column.
const TAGS_CAP: usize = 32;

/// Render every candidate into an aligned [`Row`].
///
/// Titles and tag lists are first truncated to their caps, then each column is
/// padded to the widest cell across all candidates so the date, tags and the
/// trailing ULID line up. The ULID is the display-only `suffix`, never matched.
pub fn align_candidates(candidates: &[Candidate]) -> Vec<Row> {
    // Pre-truncate the variable columns; the fixed-width date (`YYYY-MM-DD`) and
    // the 26-char ULID never need truncation here.
    let titles: Vec<String> = candidates
        .iter()
        .map(|c| truncate(&c.title, TITLE_CAP))
        .collect();
    let tags: Vec<String> = candidates.iter().map(|c| render_tags(&c.tags)).collect();

    let title_w = max_width(&titles);
    let tags_w = max_width(&tags);

    candidates
        .iter()
        .enumerate()
        .map(|(i, candidate)| {
            // A zero-width column (every title empty, or no candidate has tags)
            // is dropped entirely so it leaves no stray separator.
            let mut parts: Vec<String> = Vec::new();
            if title_w > 0 {
                parts.push(pad(&titles[i], title_w));
            }
            parts.push(format!("({})", candidate.date));
            if tags_w > 0 {
                parts.push(pad(&tags[i], tags_w));
            }
            Row {
                matchable: parts.join("  "),
                suffix: format!("  ({})", candidate.id),
            }
        })
        .collect()
}

/// The bracketed tag list (`[a, b]`), or empty when the note has no tags.
///
/// The joined tags are truncated so the bracketed result never exceeds
/// [`TAGS_CAP`]; the two bracket chars are reserved out of that budget.
fn render_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let inner = truncate(&tags.join(", "), TAGS_CAP.saturating_sub(2));
    format!("[{inner}]")
}

/// The widest cell (in chars) across a column, or zero for an empty column.
fn max_width(cells: &[String]) -> usize {
    cells.iter().map(|c| c.chars().count()).max().unwrap_or(0)
}

/// Truncate `s` to at most `max` chars, marking a cut with a trailing `…`.
///
/// Widths are counted in `char`s, not display columns, so a string of wide or
/// combining characters can still render wider than `max` cells. That matches
/// the rest of the picker's char-based truncation.
// TODO: account for Unicode display width (e.g. via unicode-width) if wide-char
// titles ever misalign the grid in practice.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Right-pad `s` with spaces to `width` chars (never truncates).
fn pad(s: &str, width: usize) -> String {
    let count = s.chars().count();
    let mut out = s.to_string();
    if count < width {
        out.push_str(&" ".repeat(width - count));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ntropy::id::Id;

    use super::*;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    /// Build a candidate from its parts; the path is irrelevant to alignment.
    fn candidate(ulid: &str, title: &str, date: &str, tags: &[&str]) -> Candidate {
        Candidate {
            id: ulid.parse::<Id>().expect("valid test ULID"),
            title: title.to_string(),
            date: date.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            path: PathBuf::new(),
        }
    }

    #[test]
    fn titles_are_padded_to_the_widest_title() {
        let rows = align_candidates(&[
            candidate(ULID_A, "short", "2026-06-25", &[]),
            candidate(ULID_B, "a much longer title", "2026-06-25", &[]),
        ]);
        // The short title is padded so both dates start at the same column.
        let date_col = |m: &str| m.find("(2026").expect("date present");
        assert_eq!(date_col(&rows[0].matchable), date_col(&rows[1].matchable));
        assert!(rows[0].matchable.starts_with("short "));
    }

    #[test]
    fn over_long_title_is_ellipsis_truncated_to_the_cap() {
        let long = "x".repeat(60);
        let rows = align_candidates(&[candidate(ULID_A, &long, "2026-06-25", &[])]);
        let title: String = rows[0]
            .matchable
            .chars()
            .take_while(|c| *c != '(')
            .collect();
        let title = title.trim_end();
        assert_eq!(title.chars().count(), TITLE_CAP);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn title_exactly_at_cap_is_not_truncated() {
        let exact = "y".repeat(TITLE_CAP);
        let rows = align_candidates(&[candidate(ULID_A, &exact, "2026-06-25", &[])]);
        assert!(rows[0].matchable.starts_with(&exact));
        assert!(!rows[0].matchable.contains('…'));
    }

    #[test]
    fn tags_are_bracketed_padded_and_aligned() {
        let rows = align_candidates(&[
            candidate(ULID_A, "t", "2026-06-25", &["work"]),
            candidate(ULID_B, "t", "2026-06-25", &["home", "urgent"]),
        ]);
        assert!(rows[0].matchable.contains("[work]"));
        assert!(rows[1].matchable.contains("[home, urgent]"));
        // Both ULID suffixes start at the same offset thanks to tag padding.
        assert_eq!(
            rows[0].matchable.chars().count(),
            rows[1].matchable.chars().count()
        );
    }

    #[test]
    fn over_long_tag_list_is_truncated_within_the_cap() {
        let many: Vec<&str> = vec!["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &many)]);
        let tags: String = rows[0]
            .matchable
            .chars()
            .skip_while(|c| *c != '[')
            .collect();
        assert!(tags.chars().count() <= TAGS_CAP);
        assert!(tags.contains('…'));
    }

    #[test]
    fn rows_without_tags_omit_the_tag_column_entirely() {
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &[])]);
        assert!(!rows[0].matchable.contains('['));
    }

    #[test]
    fn a_tagless_row_still_aligns_with_a_tagged_one() {
        let rows = align_candidates(&[
            candidate(ULID_A, "t", "2026-06-25", &["work"]),
            candidate(ULID_B, "t", "2026-06-25", &[]),
        ]);
        // The tagless row pads its (blank) tag column so both suffixes align.
        assert_eq!(
            rows[0].matchable.chars().count(),
            rows[1].matchable.chars().count()
        );
    }

    #[test]
    fn all_empty_titles_drop_the_title_column() {
        let rows = align_candidates(&[candidate(ULID_A, "", "2026-06-25", &["work"])]);
        assert!(rows[0].matchable.starts_with("(2026-06-25)"));
    }

    #[test]
    fn suffix_carries_the_dimmed_ulid() {
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &[])]);
        assert_eq!(rows[0].suffix, format!("  ({ULID_A})"));
    }

    #[test]
    fn date_is_always_present_and_fixed_width() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])]);
        assert!(rows[0].matchable.contains("(2026-06-25)"));
    }

    #[test]
    fn empty_candidate_set_yields_no_rows() {
        assert!(align_candidates(&[]).is_empty());
    }

    #[test]
    fn single_candidate_pads_to_its_own_width() {
        let rows = align_candidates(&[candidate(ULID_A, "solo", "2026-06-25", &["x"])]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].matchable.starts_with("solo  (2026-06-25)  [x]"));
    }
}
