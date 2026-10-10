// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Column alignment for picker rows (ADR 0027, ADR 0059).
//!
//! The picker shows every candidate as an aligned grid: `title │ date │ tags`.
//! Aligning the columns needs the widths of *all* candidates at once (the
//! title column is padded to the widest title), so this is a batch step over
//! the whole candidate set rather than a per-row render. The display string
//! carries the padding verbatim, which keeps the fuzzy match positions aligned
//! with what is drawn. The note's ULID is not part of the row; it travels as
//! the row's `detail`, which the picker shows in its stats line while the row
//! is selected.
//!
//! The title and tag columns split the row width the picker passes in, so a
//! title or tag list is cut with an ellipsis only when the terminal is too
//! narrow for it. The picker re-runs this step when the terminal width
//! changes. The split is a pure function of the natural column widths, the row
//! width and the title's share ([`allocate_columns`]), so its rules are tested
//! on numbers alone.

use ntropy::ops::Candidate;
use unicode_width::UnicodeWidthStr;

use super::Row;

/// The percentage of the space after the date that the title column may claim
/// when the tag lists need the rest. Whichever column needs less than its
/// share leaves the remainder to the other.
const TITLE_SHARE_PERCENT: usize = 67;
const _: () = assert!(TITLE_SHARE_PERCENT <= 100, "a share is a percentage");

/// The separator between two columns.
const SEPARATOR: &str = "  ";

/// The widest cell of each column across all candidates, before any cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Natural {
    title: usize,
    tags: usize,
    date: usize,
}

/// The cut widths a row width affords the title and tag columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Widths {
    title: usize,
    tags: usize,
}

/// Render every candidate into an aligned [`Row`] for a row `width` columns
/// wide (the terminal width less the selection pointer).
///
/// Each title is cut to the width [`allocate_columns`] gives its column and
/// padded to it, so every date starts at the same column. The tag list is the
/// last column: it is cut to its width but not padded, so a row ends where its
/// own content does. The ULID goes into the row's `detail`.
pub fn align_candidates(candidates: &[Candidate], width: usize) -> Vec<Row> {
    let widths = allocate_columns(&measure(candidates), width, TITLE_SHARE_PERCENT);

    candidates
        .iter()
        .map(|candidate| {
            // A zero-width column (every title empty, no candidate has tags, or
            // a terminal too narrow to spare the column) is dropped entirely,
            // as is a row's empty tag cell, so neither leaves a stray
            // separator.
            let mut parts: Vec<String> = Vec::new();
            if widths.title > 0 {
                parts.push(pad(&truncate(&candidate.title, widths.title), widths.title));
            }
            parts.push(date_cell(candidate));
            let tags = render_tags(&candidate.tags, widths.tags);
            if !tags.is_empty() {
                parts.push(tags);
            }
            Row {
                display: parts.join(SEPARATOR),
                detail: candidate.id.to_string(),
                search: search_text(candidate),
            }
        })
        .collect()
}

/// The natural width of every column: its widest cell across `candidates`,
/// measured on the full title and the full bracketed tag list.
fn measure(candidates: &[Candidate]) -> Natural {
    let widest = |cell: &dyn Fn(&Candidate) -> String| {
        candidates
            .iter()
            .map(|c| cell(c).width())
            .max()
            .unwrap_or(0)
    };
    Natural {
        title: widest(&|c| c.title.clone()),
        tags: widest(&|c| bracket_tags(&c.tags)),
        date: widest(&date_cell),
    }
}

/// Split a row `width` between the title and tag columns (ADR 0059).
///
/// The date is never cut; the title and tags share what is left after it and
/// the separators of the columns that exist. When both columns need more than
/// that budget, the title gets `title_share_percent` of it and the tags the
/// rest. A column that needs less than its share takes only what it needs and
/// leaves the remainder to the other, so no space sits unused while either
/// column is cut. Neither width ever exceeds the budget or its natural width,
/// and neither shrinks as `width` grows.
fn allocate_columns(natural: &Natural, width: usize, title_share_percent: usize) -> Widths {
    // The separator budget follows the natural widths. When a narrow row
    // later drops the tag column, its two separator columns go unused.
    let separator = SEPARATOR.width();
    let separators = if natural.title > 0 { separator } else { 0 }
        + if natural.tags > 0 { separator } else { 0 };
    let budget = width.saturating_sub(natural.date + separators);

    // The tags hold on to the part of their share they need, and the title
    // takes up to everything else.
    let title_share = budget * title_share_percent.min(100) / 100;
    let tags_reserve = natural.tags.min(budget - title_share);
    let title = natural.title.min(budget - tags_reserve);

    // The brackets alone take two columns, so a tag cell narrower than three
    // could show nothing of the list.
    let mut tags = natural.tags.min(budget - title);
    if tags < 3 {
        tags = 0;
    }

    Widths { title, tags }
}

/// The date cell, `(YYYY-MM-DD)`.
fn date_cell(candidate: &Candidate) -> String {
    format!("({})", candidate.date)
}

/// The full content scored against the query, in display order
/// (`title  date  tags`) but untruncated, unpadded and without the display
/// scaffolding (no brackets). Empty fields are skipped so the corpus never
/// carries a stray double space. The ULID is excluded, matching the
/// display-only `suffix`.
fn search_text(candidate: &Candidate) -> String {
    let tags = candidate.tags.join(", ");
    [
        candidate.title.as_str(),
        candidate.date.as_str(),
        tags.as_str(),
    ]
    .into_iter()
    .filter(|field| !field.is_empty())
    .collect::<Vec<_>>()
    .join("  ")
}

/// The full bracketed tag list (`[a, b]`), or empty when the note has no tags.
fn bracket_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    format!("[{}]", tags.join(", "))
}

/// The bracketed tag list cut to at most `max` display columns, the two
/// brackets included. The cut happens inside the brackets, so a cut list still
/// reads as a tag list (`[alpha…]`). Below three columns not even `[…]` fits,
/// so the cell is empty.
fn render_tags(tags: &[String], max: usize) -> String {
    if tags.is_empty() || max < 3 {
        return String::new();
    }
    let inner = truncate(&tags.join(", "), max - 2);
    format!("[{inner}]")
}

/// Truncate `s` to at most `max` display columns, marking a cut with `…`.
///
/// Widths are Unicode display columns (via `unicode-width`), so a wide CJK
/// character counts as two and a zero-width combining mark as none. The ellipsis
/// occupies one column, reserved out of the budget on a cut.
fn truncate(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = fit_prefix(s, max - 1).to_string();
    out.push('…');
    out
}

/// The longest character prefix of `s` that is at most `max` display columns
/// wide.
///
/// Width is the *string* width, the same measure `measure` and `pad` use, so
/// a cut cell lines up with the padding around it. It differs from the sum of
/// per-character widths for emoji sequences: `#` plus VS16 is one two-column
/// glyph whose characters sum to one, and a ZWJ family emoji is two columns
/// whose characters sum to more. Prefix widths are therefore not monotonic (a
/// half-built ZWJ sequence can be wider than the finished one), so every
/// character boundary is checked rather than stopping at the first prefix that
/// overflows. Only strings that do not fit pay for that scan.
pub(super) fn fit_prefix(s: &str, max: usize) -> &str {
    if s.width() <= max {
        return s;
    }
    let mut end = 0;
    for (i, c) in s.char_indices() {
        let next = i + c.len_utf8();
        if s[..next].width() <= max {
            end = next;
        }
    }
    &s[..end]
}

/// Right-pad `s` with spaces to `width` display columns (never truncates).
fn pad(s: &str, width: usize) -> String {
    let w = s.width();
    let mut out = s.to_string();
    if w < width {
        out.push_str(&" ".repeat(width - w));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ntropy::id::Id;
    use unicode_width::UnicodeWidthStr;

    use super::*;

    const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    /// A row width at which every fixture in this module fits in full.
    const WIDE: usize = 400;

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

    /// Natural widths with today's date cell (`(YYYY-MM-DD)`).
    fn natural(title: usize, tags: usize) -> Natural {
        Natural {
            title,
            tags,
            date: 12,
        }
    }

    /// The title share the allocation tests pin their numbers to. It is fixed
    /// here rather than taken from [`TITLE_SHARE_PERCENT`], so tuning the
    /// picker's split does not rewrite these expectations.
    const SPLIT: usize = 70;

    /// `(title, tags)` of an allocation at [`SPLIT`], for compact assertions.
    fn alloc(natural: &Natural, width: usize) -> (usize, usize) {
        let w = allocate_columns(natural, width, SPLIT);
        (w.title, w.tags)
    }

    /// The budget `allocate_columns` splits: the row width less the date and
    /// the separators of the columns that exist.
    fn budget(natural: &Natural, width: usize) -> usize {
        let separators =
            if natural.title > 0 { 2 } else { 0 } + if natural.tags > 0 { 2 } else { 0 };
        width.saturating_sub(natural.date + separators)
    }

    /// The display column at which the date starts (each `(` opens the date).
    fn date_column(display: &str) -> usize {
        let prefix: String = display.chars().take_while(|c| *c != '(').collect();
        prefix.width()
    }

    /// The title cell of a display row, without its padding.
    fn title_cell(display: &str) -> String {
        let cell: String = display.chars().take_while(|c| *c != '(').collect();
        cell.trim_end().to_string()
    }

    /// The tag cell of a display row.
    fn tag_cell(display: &str) -> String {
        display.chars().skip_while(|c| *c != '[').collect()
    }

    // -------------------------------------------------------------------------
    // Column allocation
    // -------------------------------------------------------------------------

    #[test]
    fn everything_fits_at_natural_widths() {
        assert_eq!(alloc(&natural(60, 40), WIDE), (60, 40));
    }

    #[test]
    fn a_long_title_and_long_tags_split_seventy_thirty() {
        // Row 118: budget 118 - 12 - 2 - 2 = 102; 70% of it is 71.
        assert_eq!(alloc(&natural(100, 100), 118), (71, 31));
    }

    #[test]
    fn a_title_that_fits_leaves_all_the_rest_to_the_tags() {
        // Row 98: budget 82. The 10-column title needs far less than its
        // share, so the tags get the other 72.
        assert_eq!(alloc(&natural(10, 150), 98), (10, 72));
    }

    #[test]
    fn tags_that_fit_leave_all_the_rest_to_the_title() {
        // Row 116: budget 100. The 10-column tag lists need less than their
        // 30, so the title gets 90 rather than 70.
        assert_eq!(alloc(&natural(100, 10), 116), (90, 10));
    }

    #[test]
    fn the_title_has_no_fixed_cap() {
        assert_eq!(alloc(&natural(200, 10), WIDE), (200, 10));
    }

    #[test]
    fn a_vault_like_the_screenshot_fits_in_full_at_218_columns() {
        // Titles up to 83 columns, tag lists up to 106; the row is the
        // terminal width less the two-column pointer.
        assert_eq!(alloc(&natural(83, 106), 216), (83, 106));
        assert_eq!(alloc(&natural(83, 106), 118), (71, 31));
        assert_eq!(alloc(&natural(83, 106), 78), (43, 19));
    }

    #[test]
    fn narrow_rows_keep_the_split() {
        let nat = natural(60, 40);
        assert_eq!(alloc(&nat, 0), (0, 0));
        assert_eq!(alloc(&nat, 26), (7, 3));
        assert_eq!(alloc(&nat, 30), (9, 5));
        assert_eq!(alloc(&nat, 38), (15, 7));
    }

    #[test]
    fn the_title_share_is_a_parameter() {
        // Row 118: budget 102.
        let nat = natural(100, 100);
        assert_eq!(
            allocate_columns(&nat, 118, 50),
            Widths {
                title: 51,
                tags: 51
            }
        );
        assert_eq!(
            allocate_columns(&nat, 118, 100),
            Widths {
                title: 100,
                tags: 0
            }
        );
        assert_eq!(
            allocate_columns(&nat, 118, 0),
            Widths {
                title: 2,
                tags: 100
            }
        );
    }

    #[test]
    fn rows_are_laid_out_at_the_configured_share() {
        let cands = [candidate(
            ULID_A,
            &"x".repeat(100),
            "2026-06-25",
            &["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta"],
        )];
        let width = 118;
        let expected = allocate_columns(&measure(&cands), width, TITLE_SHARE_PERCENT);
        let rows = align_candidates(&cands, width);
        assert_eq!(title_cell(&rows[0].display).width(), expected.title);
        assert_eq!(tag_cell(&rows[0].display).width(), expected.tags);
    }

    #[test]
    fn a_tag_column_under_three_columns_is_dropped() {
        // Row 20: budget 4 splits 2 / 2, and two columns cannot hold the
        // brackets around a single character.
        assert_eq!(alloc(&natural(60, 40), 20), (2, 0));
    }

    #[test]
    fn each_column_gets_at_least_its_share_when_it_needs_it() {
        let titles = [0, 1, 5, 20, 24, 30, 60, 83, 100, 200];
        let tags = [0, 3, 4, 5, 6, 12, 20, 40, 106, 150];
        for &title_nat in &titles {
            for &tags_nat in &tags {
                let nat = natural(title_nat, tags_nat);
                for width in 0..WIDE {
                    let (title, tags) = alloc(&nat, width);
                    let budget = budget(&nat, width);
                    let title_share = budget * SPLIT / 100;
                    let tags_share = budget - title_share;
                    assert!(title >= title_nat.min(title_share), "{nat:?} at {width}");
                    if tags_nat.min(tags_share) >= 3 {
                        assert!(tags >= tags_nat.min(tags_share), "{nat:?} at {width}");
                    }
                }
            }
        }
    }

    #[test]
    fn widening_never_shrinks_a_column() {
        let titles = [0, 1, 5, 20, 24, 30, 60, 83, 100, 200];
        let tags = [0, 3, 4, 5, 6, 12, 20, 40, 106, 150];
        for &title_nat in &titles {
            for &tags_nat in &tags {
                let nat = natural(title_nat, tags_nat);
                let mut previous = alloc(&nat, 0);
                for width in 1..WIDE {
                    let current = alloc(&nat, width);
                    assert!(
                        current.0 >= previous.0 && current.1 >= previous.1,
                        "{nat:?} shrinks at {width}: {previous:?} -> {current:?}"
                    );
                    previous = current;
                }
            }
        }
    }

    #[test]
    fn allocated_widths_never_exceed_the_budget_or_the_natural_widths() {
        for &(title_nat, tags_nat) in &[(60, 40), (10, 150), (0, 40), (200, 0), (5, 5)] {
            let nat = natural(title_nat, tags_nat);
            for width in 0..WIDE {
                let (title, tags) = alloc(&nat, width);
                assert!(title + tags <= budget(&nat, width), "{nat:?} at {width}");
                assert!(title <= title_nat && tags <= tags_nat, "{nat:?} at {width}");
            }
        }
    }

    #[test]
    fn without_titles_no_title_separator_is_budgeted() {
        // Row 54: budget 54 - 12 - 2 = 40 holds the full tag list. A budgeted
        // title separator would leave 38.
        assert_eq!(alloc(&natural(0, 40), 54), (0, 40));
    }

    #[test]
    fn without_tags_no_tag_separator_is_budgeted() {
        // Row 74: budget 74 - 12 - 2 = 60 holds the full title. A budgeted
        // tag separator would leave 58.
        assert_eq!(alloc(&natural(60, 0), 74), (60, 0));
    }
    // -------------------------------------------------------------------------
    // Measuring
    // -------------------------------------------------------------------------

    #[test]
    fn measure_takes_the_widest_untruncated_cell_of_each_column() {
        let long_title = "t".repeat(120);
        let nat = measure(&[
            candidate(ULID_A, "short", "2026-06-25", &["work"]),
            candidate(ULID_B, &long_title, "2026-06-25", &["home", "urgent"]),
        ]);
        assert_eq!(nat.title, 120);
        assert_eq!(nat.tags, "[home, urgent]".width());
        assert_eq!(nat.date, "(2026-06-25)".width());
    }

    #[test]
    fn measure_of_no_candidates_is_all_zero() {
        let nat = measure(&[]);
        assert_eq!((nat.title, nat.tags, nat.date), (0, 0, 0));
    }

    // -------------------------------------------------------------------------
    // Tag cells
    // -------------------------------------------------------------------------

    #[test]
    fn bracket_tags_joins_without_truncating() {
        let tags: Vec<String> = ["alpha", "beta"].iter().map(|t| t.to_string()).collect();
        assert_eq!(bracket_tags(&tags), "[alpha, beta]");
        assert_eq!(bracket_tags(&[]), "");
    }

    #[test]
    fn render_tags_keeps_a_fitting_list() {
        let tags: Vec<String> = ["alpha", "beta"].iter().map(|t| t.to_string()).collect();
        assert_eq!(render_tags(&tags, 13), "[alpha, beta]");
        assert_eq!(render_tags(&tags, WIDE), "[alpha, beta]");
    }

    #[test]
    fn render_tags_cuts_the_list_inside_its_brackets() {
        let tags: Vec<String> = ["alpha", "beta"].iter().map(|t| t.to_string()).collect();
        assert_eq!(render_tags(&tags, 8), "[alpha…]");
        assert_eq!(render_tags(&tags, 3), "[…]");
    }

    #[test]
    fn render_tags_is_empty_below_three_columns() {
        let tags = vec!["alpha".to_string()];
        assert_eq!(render_tags(&tags, 2), "");
        assert_eq!(render_tags(&tags, 0), "");
        assert_eq!(render_tags(&[], WIDE), "");
    }

    // -------------------------------------------------------------------------
    // Row assembly
    // -------------------------------------------------------------------------

    #[test]
    fn titles_are_padded_to_the_widest_title() {
        let rows = align_candidates(
            &[
                candidate(ULID_A, "short", "2026-06-25", &[]),
                candidate(ULID_B, "a much longer title", "2026-06-25", &[]),
            ],
            WIDE,
        );
        // The short title is padded so both dates start at the same column.
        assert_eq!(date_column(&rows[0].display), date_column(&rows[1].display));
        assert!(rows[0].display.starts_with("short "));
    }

    #[test]
    fn an_over_long_title_is_ellipsis_truncated_to_its_column() {
        let cands = [candidate(ULID_A, &"x".repeat(60), "2026-06-25", &[])];
        let width = 60;
        let title_w = allocate_columns(&measure(&cands), width, TITLE_SHARE_PERCENT).title;
        assert!(title_w < 60);
        let title = title_cell(&align_candidates(&cands, width)[0].display);
        assert_eq!(title.width(), title_w);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn a_long_title_on_a_wide_terminal_shows_in_full() {
        let long = "x".repeat(70);
        let rows = align_candidates(&[candidate(ULID_A, &long, "2026-06-25", &[])], WIDE);
        assert!(rows[0].display.starts_with(&long));
        assert!(!rows[0].display.contains('…'));
    }

    #[test]
    fn a_very_long_title_shows_in_full_when_the_terminal_fits_it() {
        let long = "y".repeat(150);
        let rows = align_candidates(&[candidate(ULID_A, &long, "2026-06-25", &[])], WIDE);
        assert!(rows[0].display.starts_with(&long));
        assert!(!rows[0].display.contains('…'));
    }

    #[test]
    fn tags_are_bracketed_and_end_the_row_unpadded() {
        let rows = align_candidates(
            &[
                candidate(ULID_A, "t", "2026-06-25", &["work"]),
                candidate(ULID_B, "t", "2026-06-25", &["home", "urgent"]),
            ],
            WIDE,
        );
        assert!(rows[0].display.ends_with("(2026-06-25)  [work]"));
        assert!(rows[1].display.ends_with("(2026-06-25)  [home, urgent]"));
    }

    #[test]
    fn an_over_long_tag_list_is_truncated_to_its_column() {
        let many = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
        let cands = [candidate(ULID_A, "t", "2026-06-25", &many)];
        let width = 40;
        let nat = measure(&cands);
        let tags_w = allocate_columns(&nat, width, TITLE_SHARE_PERCENT).tags;
        assert!(tags_w < nat.tags);
        let tags = tag_cell(&align_candidates(&cands, width)[0].display);
        assert_eq!(tags.width(), tags_w);
        assert!(tags.contains('…'));
    }

    #[test]
    fn rows_without_tags_omit_the_tag_column_entirely() {
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &[])], WIDE);
        assert!(!rows[0].display.contains('['));
        assert!(!rows[0].display.ends_with(' '));
    }

    #[test]
    fn a_tagless_row_among_tagged_ones_ends_at_its_date() {
        let rows = align_candidates(
            &[
                candidate(ULID_A, "t", "2026-06-25", &["work"]),
                candidate(ULID_B, "t", "2026-06-25", &[]),
            ],
            WIDE,
        );
        assert_eq!(rows[1].display, "t  (2026-06-25)");
    }

    #[test]
    fn all_empty_titles_drop_the_title_column() {
        let rows = align_candidates(&[candidate(ULID_A, "", "2026-06-25", &["work"])], WIDE);
        assert!(rows[0].display.starts_with("(2026-06-25)"));
    }

    #[test]
    fn detail_carries_the_ulid() {
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &[])], WIDE);
        assert_eq!(rows[0].detail, ULID_A);
        assert!(!rows[0].display.contains(ULID_A));
    }

    #[test]
    fn detail_carries_the_ulid_at_any_width() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])], 0);
        assert_eq!(rows[0].detail, ULID_A);
    }

    #[test]
    fn every_row_fits_its_width_once_the_date_does() {
        let cands = [
            candidate(
                ULID_A,
                &"x".repeat(60),
                "2026-06-25",
                &["alpha", "beta", "gamma"],
            ),
            candidate(ULID_B, "日本語のタイトル", "2026-06-25", &["work"]),
        ];
        // Below the date and its separators nothing but the edge cut helps.
        for width in 16..WIDE {
            for row in align_candidates(&cands, width) {
                let used = row.display.width();
                assert!(used <= width, "{used} columns at width {width}");
            }
        }
    }

    #[test]
    fn a_zero_row_width_leaves_only_the_date() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])], 0);
        assert_eq!(rows[0].display, "(2026-06-25)");
        assert_eq!(rows[0].detail, ULID_A);
    }

    #[test]
    fn date_is_always_present_and_fixed_width() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])], WIDE);
        assert!(rows[0].display.contains("(2026-06-25)"));
    }

    #[test]
    fn empty_candidate_set_yields_no_rows() {
        assert!(align_candidates(&[], WIDE).is_empty());
    }

    #[test]
    fn single_candidate_pads_to_its_own_width() {
        let rows = align_candidates(&[candidate(ULID_A, "solo", "2026-06-25", &["x"])], WIDE);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].display, "solo  (2026-06-25)  [x]");
    }

    #[test]
    fn a_wide_title_truncates_by_display_width() {
        // 30 CJK chars span 60 display columns.
        let cands = [candidate(ULID_A, &"ナ".repeat(30), "2026-06-25", &[])];
        let width = 50;
        let title_w = allocate_columns(&measure(&cands), width, TITLE_SHARE_PERCENT).title;
        assert!(title_w < 60);
        let rows = align_candidates(&cands, width);
        let title = title_cell(&rows[0].display);
        assert!(title.ends_with('…'));
        assert!(title.width() <= title_w);
        // A two-column character never straddles the cut, and the padding
        // still fills the column up to the date.
        assert_eq!(date_column(&rows[0].display), title_w + 2);
    }

    #[test]
    fn wide_and_ascii_titles_align_by_display_width() {
        let rows = align_candidates(
            &[
                candidate(ULID_A, "日本語", "2026-06-25", &[]),
                candidate(ULID_B, "ascii", "2026-06-25", &[]),
            ],
            WIDE,
        );
        // Despite different char counts, both dates begin at the same column.
        assert_eq!(date_column(&rows[0].display), date_column(&rows[1].display));
        // The CJK title (3 chars, 6 columns) is the widest, so it sets the column.
        assert_eq!(date_column(&rows[0].display), 6 + 2);
    }

    // -------------------------------------------------------------------------
    // String-width cuts
    // -------------------------------------------------------------------------

    #[test]
    fn truncate_measures_emoji_presentation_by_string_width() {
        // `#` followed by VS16 renders as one two-column emoji, while its
        // per-character widths sum to one. The cut must honour the string
        // width or the cell spills past its column.
        let cut = truncate("#\u{FE0F}xyz", 3);
        assert!(cut.width() <= 3, "{cut:?} is {} columns", cut.width());
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn an_emoji_presentation_title_stays_within_its_column() {
        let cands = [
            candidate(ULID_A, &"#\u{FE0F}".repeat(30), "2026-06-25", &[]),
            candidate(ULID_B, "plain", "2026-06-25", &[]),
        ];
        let width = 60;
        let title_w = allocate_columns(&measure(&cands), width, TITLE_SHARE_PERCENT).title;
        let rows = align_candidates(&cands, width);
        assert_eq!(date_column(&rows[0].display), title_w + 2);
        assert_eq!(date_column(&rows[0].display), date_column(&rows[1].display));
    }

    #[test]
    fn fit_prefix_keeps_a_zwj_sequence_that_fits_by_string_width() {
        // The family emoji is one two-column glyph, so with the trailing `x`
        // the whole string is three columns wide, although its characters'
        // widths sum to more.
        let s = "🧑\u{200D}🤝\u{200D}🧑x";
        assert_eq!(s.width(), 3);
        assert_eq!(fit_prefix(s, 3), s);
    }

    #[test]
    fn fit_prefix_stops_at_the_widest_prefix_that_fits() {
        assert_eq!(fit_prefix("abcdef", 4), "abcd");
        // A wide character that would straddle the edge is dropped whole.
        assert_eq!(fit_prefix("日本語", 5), "日本");
        assert_eq!(fit_prefix("abc", 0), "");
    }

    #[test]
    fn fit_prefix_returns_a_fitting_string_unchanged() {
        assert_eq!(fit_prefix("abc", 3), "abc");
        assert_eq!(fit_prefix("abc", 10), "abc");
        assert_eq!(fit_prefix("", 0), "");
    }

    #[test]
    fn fit_prefix_finds_a_complete_zwj_sequence_past_a_wider_partial_one() {
        // Two regional-indicator pairs joined by a ZWJ form one two-column
        // ligature, but the prefix that stops after the third indicator is
        // three columns wide. A scan that stopped at the first prefix over
        // budget would keep only the first pair and the joiner.
        let joined = "🇦🇦\u{200D}🇦🇦";
        assert_eq!(joined.width(), 2);
        assert_eq!("🇦🇦\u{200D}🇦".width(), 3);
        assert_eq!(fit_prefix(&format!("{joined}x"), 2), joined);
    }

    #[test]
    fn fit_prefix_never_exceeds_the_budget_inside_a_zwj_sequence() {
        // Cutting into the family emoji leaves a partial sequence whose string
        // width differs from the complete glyph's; whatever prefix is kept
        // must still fit.
        let s = "🧑\u{200D}🤝\u{200D}🧑x";
        for max in 0..=s.width() {
            let kept = fit_prefix(s, max);
            assert!(kept.width() <= max, "{kept:?} exceeds {max}");
        }
    }

    #[test]
    fn truncate_leaves_a_fitting_string_alone() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("", 0), "");
    }

    #[test]
    fn truncate_to_zero_columns_is_empty() {
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn truncate_to_one_column_is_only_the_ellipsis() {
        assert_eq!(truncate("abc", 1), "…");
    }

    // -------------------------------------------------------------------------
    // Full-content search corpus
    // -------------------------------------------------------------------------

    #[test]
    fn search_carries_the_full_title_when_display_truncates_it() {
        // A title cut for display keeps its tail searchable.
        let long = format!("{}TAILWORD", "x".repeat(60));
        let rows = align_candidates(&[candidate(ULID_A, &long, "2026-06-25", &[])], 60);
        assert!(rows[0].display.contains('…'));
        assert!(!rows[0].display.contains("TAILWORD"));
        assert!(rows[0].search.contains("TAILWORD"));
        assert!(rows[0].search.starts_with(&long));
    }

    #[test]
    fn search_carries_the_full_tag_list_when_display_truncates_it() {
        // A tag clipped from the display column remains in the search corpus.
        let many = ["alpha", "beta", "gamma", "delta", "epsilon", "lasttag"];
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &many)], 40);
        assert!(rows[0].display.contains('…'));
        assert!(!rows[0].display.contains("lasttag"));
        assert!(rows[0].search.contains("lasttag"));
    }

    #[test]
    fn search_contains_the_date() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])], WIDE);
        assert!(rows[0].search.contains("2026-06-25"));
    }

    #[test]
    fn search_is_plain_without_brackets_or_padding() {
        // The corpus joins fields with a single double space and carries no
        // display scaffolding: tags appear unbracketed and there is no run of
        // padding spaces.
        let rows = align_candidates(
            &[candidate(ULID_A, "note", "2026-06-25", &["work", "home"])],
            WIDE,
        );
        assert_eq!(rows[0].search, "note  2026-06-25  work, home");
    }

    #[test]
    fn search_skips_empty_fields() {
        // A note with no title and no tags yields just the date, with no leading
        // or trailing separator.
        let rows = align_candidates(&[candidate(ULID_A, "", "2026-06-25", &[])], WIDE);
        assert_eq!(rows[0].search, "2026-06-25");
    }
}
