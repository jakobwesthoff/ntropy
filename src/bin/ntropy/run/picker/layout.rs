// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Column alignment for picker rows (ADR 0027, ADR 0059).
//!
//! The picker shows every candidate as an aligned grid: `title │ date │ tags`
//! followed by the display-only ULID. Aligning the columns needs the widths of
//! *all* candidates at once (the title column is padded to the widest title,
//! and so on), so this is a batch step over the whole candidate set rather
//! than a per-row render. The display string carries the padding verbatim,
//! which keeps the fuzzy match positions aligned with what is drawn.
//!
//! The title and tag columns are sized from the row width the picker passes
//! in, so a title or tag list is cut with an ellipsis only when the terminal
//! is too narrow for it. The picker re-runs this step when the terminal width
//! changes. The allocation is a pure function of the natural column widths and
//! the row width ([`allocate_columns`]), so its rules are tested on numbers
//! alone.

use ntropy::ops::Candidate;
use unicode_width::UnicodeWidthStr;

use super::Row;

/// The widest the title column grows, however wide the terminal. One outlier
/// title would otherwise push the date column of every row far to the right.
const TITLE_MAX: usize = 80;
/// The title width that wins over the tags on a narrow terminal (or the
/// widest title, if shorter).
const TITLE_FLOOR: usize = 24;
/// The separator between two columns.
const SEPARATOR: &str = "  ";

/// The widest cell of each column across all candidates, before any cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Natural {
    title: usize,
    tags: usize,
    date: usize,
    suffix: usize,
}

/// The columns a row width affords: the cut widths of the title and tag
/// columns, and whether the ULID suffix fits after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Widths {
    title: usize,
    tags: usize,
    show_suffix: bool,
}

/// Render every candidate into an aligned [`Row`] for a row `width` columns
/// wide (the terminal width less the selection pointer).
///
/// Each title and tag list is cut to the width [`allocate_columns`] gives its
/// column, then padded to it, so the date, tags and the trailing ULID line up.
/// The ULID is the display-only `suffix`, never matched, and is left empty
/// when it does not fit.
pub fn align_candidates(candidates: &[Candidate], width: usize) -> Vec<Row> {
    let widths = allocate_columns(&measure(candidates), width);

    candidates
        .iter()
        .map(|candidate| {
            // A zero-width column (every title empty, no candidate has tags, or
            // a terminal too narrow to spare the column) is dropped entirely so
            // it leaves no stray separator.
            let mut parts: Vec<String> = Vec::new();
            if widths.title > 0 {
                parts.push(pad(&truncate(&candidate.title, widths.title), widths.title));
            }
            parts.push(date_cell(candidate));
            if widths.tags > 0 {
                parts.push(pad(&render_tags(&candidate.tags, widths.tags), widths.tags));
            }
            Row {
                display: parts.join(SEPARATOR),
                suffix: if widths.show_suffix {
                    suffix_cell(candidate)
                } else {
                    String::new()
                },
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
        suffix: widest(&suffix_cell),
    }
}

/// Split a row `width` between the title and tag columns (ADR 0059).
///
/// The date is never cut; the title and tags share what is left after it and
/// the separators of the columns that exist. The tags first get a third of
/// that budget (at most their natural width), the title takes what it wants
/// of the rest up to [`TITLE_MAX`], and the tags then get whatever the title
/// leaves. On a narrow terminal the title takes up to [`TITLE_FLOOR`] columns
/// from the tags' share. The ULID is shown only in space left over once both
/// columns have their full width, so widening the terminal never shrinks a
/// column to make room for it. No width ever exceeds the budget.
fn allocate_columns(natural: &Natural, width: usize) -> Widths {
    let title_want = natural.title.min(TITLE_MAX);
    let tags_want = natural.tags;

    // The separator budget follows the natural widths. When a narrow row
    // later drops the tag column, its two separator columns go unused.
    let separator = SEPARATOR.width();
    let separators = if natural.title > 0 { separator } else { 0 }
        + if natural.tags > 0 { separator } else { 0 };
    let budget = width.saturating_sub(natural.date + separators);

    let first_tags = tags_want.min(budget / 3);
    let mut title = title_want.min(budget.saturating_sub(first_tags));
    if title < natural.title.min(TITLE_FLOOR) {
        title = natural.title.min(TITLE_FLOOR).min(budget);
    }

    // The brackets alone take two columns, so a tag cell narrower than three
    // could show nothing of the list.
    let mut tags = tags_want.min(budget - title);
    if tags < 3 {
        tags = 0;
    }

    Widths {
        title,
        tags,
        show_suffix: budget - title - tags >= natural.suffix,
    }
}

/// The date cell, `(YYYY-MM-DD)`.
fn date_cell(candidate: &Candidate) -> String {
    format!("({})", candidate.date)
}

/// The ULID suffix cell, carrying its own two-space separator so it can be
/// left out without leaving one behind.
fn suffix_cell(candidate: &Candidate) -> String {
    format!("{SEPARATOR}({})", candidate.id)
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

    /// Natural widths with today's date (`(YYYY-MM-DD)`) and ULID suffix
    /// (`"  (" + 26 + ")"`) cells.
    fn natural(title: usize, tags: usize) -> Natural {
        Natural {
            title,
            tags,
            date: 12,
            suffix: 30,
        }
    }

    /// `(title, tags, show_suffix)` of an allocation, for compact assertions.
    fn alloc(natural: &Natural, width: usize) -> (usize, usize, bool) {
        let w = allocate_columns(natural, width);
        (w.title, w.tags, w.show_suffix)
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

    /// The tag cell of a display row, without its padding.
    fn tag_cell(display: &str) -> String {
        let cell: String = display.chars().skip_while(|c| *c != '[').collect();
        cell.trim_end().to_string()
    }

    // -------------------------------------------------------------------------
    // Column allocation
    // -------------------------------------------------------------------------

    #[test]
    fn everything_fits_at_natural_widths_with_the_ulid() {
        assert_eq!(alloc(&natural(60, 40), WIDE), (60, 40, true));
    }

    #[test]
    fn tags_are_held_to_their_share_while_the_title_is_truncated() {
        // Row 118: budget 118 - 12 - 2 - 2 = 102, a third of it is 34.
        let (title, tags, show_suffix) = alloc(&natural(100, 100), 118);
        assert_eq!(tags, 34);
        assert_eq!(title, 68);
        assert!(!show_suffix);
    }

    #[test]
    fn the_title_is_held_at_title_max_on_a_very_wide_terminal() {
        let (title, tags, show_suffix) = alloc(&natural(200, 10), WIDE);
        assert_eq!(title, TITLE_MAX);
        assert_eq!(tags, 10);
        assert!(show_suffix);
    }

    #[test]
    fn space_the_title_does_not_use_goes_to_the_tags() {
        // Row 98: budget 82. The tags' first share is 27, the 10-column title
        // leaves 72 of the budget, and the tags take all of it.
        assert_eq!(alloc(&natural(10, 150), 98), (10, 72, false));
    }

    #[test]
    fn the_ulid_shows_only_once_title_and_tags_fit_in_full() {
        // 60 + 2 + 12 + 2 + 40 + 30 = 146.
        assert_eq!(alloc(&natural(60, 40), 145), (60, 40, false));
        assert_eq!(alloc(&natural(60, 40), 146), (60, 40, true));
    }

    #[test]
    fn widening_never_shrinks_a_column_or_hides_the_ulid() {
        let titles = [0, 1, 5, 20, 23, 24, 25, 30, 60, 79, 80, 81, 100, 200];
        let tags = [0, 3, 4, 5, 6, 12, 13, 20, 40, 150];
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
                    assert!(
                        current.2 || !previous.2,
                        "{nat:?} hides the ULID at {width}"
                    );
                    previous = current;
                }
            }
        }
    }

    #[test]
    fn the_ulid_never_shows_while_a_column_is_cut_below_its_want() {
        let titles = [0, 5, 24, 60, 80, 100, 200];
        let tags = [0, 3, 12, 40, 150];
        for &title_nat in &titles {
            for &tags_nat in &tags {
                let nat = natural(title_nat, tags_nat);
                for width in 0..WIDE {
                    let (title, tags, show_suffix) = alloc(&nat, width);
                    if show_suffix {
                        assert_eq!(title, title_nat.min(TITLE_MAX), "{nat:?} at {width}");
                        assert_eq!(tags, tags_nat, "{nat:?} at {width}");
                    }
                }
            }
        }
    }

    #[test]
    fn allocated_widths_never_exceed_the_budget() {
        for &(title_nat, tags_nat) in &[(60, 40), (10, 150), (0, 40), (200, 0), (5, 5)] {
            let nat = natural(title_nat, tags_nat);
            for width in 0..WIDE {
                let (title, tags, _) = alloc(&nat, width);
                let separators =
                    if title_nat > 0 { 2 } else { 0 } + if tags_nat > 0 { 2 } else { 0 };
                let budget = width.saturating_sub(nat.date + separators);
                assert!(title + tags <= budget, "{nat:?} at {width}");
            }
        }
    }

    #[test]
    fn without_titles_no_title_separator_is_budgeted() {
        // Row 54: budget 54 - 12 - 2 = 40 holds the full tag list. A budgeted
        // title separator would leave 38.
        assert_eq!(alloc(&natural(0, 40), 54), (0, 40, false));
    }

    #[test]
    fn without_tags_no_tag_separator_is_budgeted() {
        // Row 74: budget 74 - 12 - 2 = 60 holds the full title. A budgeted
        // tag separator would leave 58.
        assert_eq!(alloc(&natural(60, 0), 74), (60, 0, false));
    }

    #[test]
    fn without_titles_the_ulid_stays_hidden_while_the_tags_are_cut() {
        // 12 + 2 + 40 + 30 = 84.
        assert_eq!(alloc(&natural(0, 40), 83), (0, 40, false));
        assert_eq!(alloc(&natural(0, 40), 53), (0, 39, false));
        assert_eq!(alloc(&natural(0, 40), 84), (0, 40, true));
    }

    #[test]
    fn narrow_rows_give_the_title_its_floor_before_the_tags() {
        let nat = natural(60, 40);
        assert_eq!(alloc(&nat, 0), (0, 0, false));
        assert_eq!(alloc(&nat, 38), (22, 0, false));
        // Budget 25: the title takes 24, and the single column left for the
        // tags cannot hold their brackets.
        assert_eq!(alloc(&nat, 41), (24, 0, false));
        assert_eq!(alloc(&nat, 46), (24, 6, false));
        assert_eq!(alloc(&nat, 50), (24, 10, false));
        assert_eq!(alloc(&nat, 54), (26, 12, false));
    }

    #[test]
    fn a_title_shorter_than_the_floor_claims_only_its_own_width() {
        // Row 20: budget 4. A 10-column title wants less than the floor and
        // still gets everything the budget holds.
        assert_eq!(alloc(&natural(10, 40), 20), (4, 0, false));
        assert_eq!(alloc(&natural(10, 40), 40), (10, 14, false));
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
        assert_eq!(nat.suffix, format!("  ({ULID_A})").width());
    }

    #[test]
    fn measure_of_no_candidates_is_all_zero() {
        let nat = measure(&[]);
        assert_eq!((nat.title, nat.tags, nat.date, nat.suffix), (0, 0, 0, 0));
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
        let title_w = allocate_columns(&measure(&cands), width).title;
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
    fn a_title_exactly_at_title_max_is_not_truncated() {
        let exact = "y".repeat(TITLE_MAX);
        let rows = align_candidates(&[candidate(ULID_A, &exact, "2026-06-25", &[])], WIDE);
        assert!(rows[0].display.starts_with(&exact));
        assert!(!rows[0].display.contains('…'));
    }

    #[test]
    fn a_title_one_past_title_max_is_cut_to_title_max() {
        let long = "y".repeat(TITLE_MAX + 1);
        let rows = align_candidates(&[candidate(ULID_A, &long, "2026-06-25", &[])], WIDE);
        let title = title_cell(&rows[0].display);
        assert_eq!(title.width(), TITLE_MAX);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn a_title_past_title_max_is_cut_while_the_ulid_shows() {
        let rows = align_candidates(
            &[candidate(ULID_A, &"z".repeat(100), "2026-06-25", &[])],
            WIDE,
        );
        assert_eq!(title_cell(&rows[0].display).width(), TITLE_MAX);
        assert_eq!(rows[0].suffix, format!("  ({ULID_A})"));
    }

    #[test]
    fn tags_are_bracketed_padded_and_aligned() {
        let rows = align_candidates(
            &[
                candidate(ULID_A, "t", "2026-06-25", &["work"]),
                candidate(ULID_B, "t", "2026-06-25", &["home", "urgent"]),
            ],
            WIDE,
        );
        assert!(rows[0].display.contains("[work]"));
        assert!(rows[1].display.contains("[home, urgent]"));
        // Both ULID suffixes start at the same offset thanks to tag padding.
        assert_eq!(rows[0].display.width(), rows[1].display.width());
    }

    #[test]
    fn an_over_long_tag_list_is_truncated_to_its_column() {
        let many = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
        let cands = [candidate(ULID_A, "t", "2026-06-25", &many)];
        let width = 40;
        let nat = measure(&cands);
        let tags_w = allocate_columns(&nat, width).tags;
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
    fn a_tagless_row_still_aligns_with_a_tagged_one() {
        let rows = align_candidates(
            &[
                candidate(ULID_A, "t", "2026-06-25", &["work"]),
                candidate(ULID_B, "t", "2026-06-25", &[]),
            ],
            WIDE,
        );
        // The tagless row pads its (blank) tag column so both suffixes align.
        assert_eq!(rows[0].display.width(), rows[1].display.width());
    }

    #[test]
    fn all_empty_titles_drop_the_title_column() {
        let rows = align_candidates(&[candidate(ULID_A, "", "2026-06-25", &["work"])], WIDE);
        assert!(rows[0].display.starts_with("(2026-06-25)"));
    }

    #[test]
    fn suffix_carries_the_dimmed_ulid() {
        let rows = align_candidates(&[candidate(ULID_A, "t", "2026-06-25", &[])], WIDE);
        assert_eq!(rows[0].suffix, format!("  ({ULID_A})"));
    }

    #[test]
    fn suffix_is_empty_when_the_ulid_does_not_fit() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])], 40);
        assert!(rows[0].suffix.is_empty());
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
                let used = row.display.width() + row.suffix.width();
                assert!(used <= width, "{used} columns at width {width}");
            }
        }
    }

    #[test]
    fn a_zero_row_width_leaves_only_the_date() {
        let rows = align_candidates(&[candidate(ULID_A, "title", "2026-06-25", &["work"])], 0);
        assert_eq!(rows[0].display, "(2026-06-25)");
        assert!(rows[0].suffix.is_empty());
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
        assert!(rows[0].display.starts_with("solo  (2026-06-25)  [x]"));
    }

    #[test]
    fn a_wide_title_truncates_by_display_width() {
        // 30 CJK chars span 60 display columns.
        let cands = [candidate(ULID_A, &"ナ".repeat(30), "2026-06-25", &[])];
        let width = 50;
        let title_w = allocate_columns(&measure(&cands), width).title;
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
        let title_w = allocate_columns(&measure(&cands), width).title;
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
