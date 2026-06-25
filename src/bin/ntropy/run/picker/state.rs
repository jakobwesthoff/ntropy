// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The picker's pure interaction state, free of any terminal I/O.
//!
//! Everything that decides *what* the picker shows lives here: fuzzy filtering
//! and ranking, the query string, the selection cursor, and the scroll
//! viewport. The terminal loop in the parent module only reads this state to
//! draw and feeds key events back into it, so the entire behaviour is unit
//! testable without a TTY (ADR 0021). Snapshot tests drive [`PickerState`]
//! directly via [`PickerState::debug_render`].

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

use super::Row;

/// One ranked result: the item it points at and the matched character
/// positions within that item's rendered row.
struct Scored {
    /// Index into [`PickerState::rows`] / `items`.
    item: usize,
    /// Char positions in the rendered row that the query matched, ascending.
    positions: Vec<u32>,
}

/// A row currently inside the viewport, handed to the renderer (or a test).
pub struct VisibleRow<'a> {
    /// The matchable, highlightable part of the row (shown first).
    pub matchable: &'a str,
    /// Trailing text shown but never matched or highlighted (e.g. an id).
    pub suffix: &'a str,
    /// Char positions in `matchable` that matched the query, ascending.
    pub positions: &'a [u32],
    /// Whether this row is the current selection.
    pub selected: bool,
}

/// The picker's interaction state over an owned set of items of type `T`.
pub struct PickerState<T> {
    /// The candidate items, index-aligned with `matchable`/`suffix`/`haystacks`.
    items: Vec<T>,
    /// The matchable, highlightable text per item.
    matchable: Vec<String>,
    /// The trailing display-only text per item (shown, never matched).
    suffix: Vec<String>,
    /// Pre-converted match haystacks per item (the same text as `matchable`).
    haystacks: Vec<Utf32String>,
    /// Reused fuzzy matcher; allocates a large scratch buffer, so it is kept.
    matcher: Matcher,
    /// The current query text.
    query: String,
    /// Current filtered, ranked results (best first).
    scored: Vec<Scored>,
    /// Selection cursor as an index into `scored`.
    selected: usize,
    /// Index into `scored` of the first visible row (scroll offset).
    offset: usize,
    /// Number of list rows the viewport can show (always at least 1).
    height: usize,
}

impl<T> PickerState<T> {
    /// Build the state from `items` and their pre-rendered `rows`.
    ///
    /// `rows` is index-aligned with `items` (one [`Row`] per item); the renderer
    /// produces them in one batch so it can align columns across every item.
    /// `height` is the number of list rows the viewport can show; it is clamped
    /// to at least one so movement and scrolling always have room.
    pub fn new(items: Vec<T>, rows: Vec<Row>, height: usize) -> Self {
        debug_assert_eq!(items.len(), rows.len(), "one row per item");
        let haystacks: Vec<Utf32String> = rows
            .iter()
            .map(|r| Utf32String::from(r.matchable.as_str()))
            .collect();
        let mut matchable: Vec<String> = Vec::with_capacity(rows.len());
        let mut suffix: Vec<String> = Vec::with_capacity(rows.len());
        for row in rows {
            matchable.push(row.matchable);
            suffix.push(row.suffix);
        }
        let mut state = Self {
            items,
            matchable,
            suffix,
            haystacks,
            matcher: Matcher::new(Config::DEFAULT),
            query: String::new(),
            scored: Vec::new(),
            selected: 0,
            offset: 0,
            height: height.max(1),
        };
        state.recompute();
        state
    }

    /// Recompute the ranked result set for the current query.
    ///
    /// An empty query keeps every item in its original (newest-first) order with
    /// no highlights. A non-empty query is fuzzy-matched against each row; rows
    /// that match are ranked by score descending, ties broken by original index
    /// so equal-score rows stay newest-first. The cursor and scroll reset to the
    /// top because the result set has changed underneath them.
    fn recompute(&mut self) {
        self.selected = 0;
        self.offset = 0;

        let query = self.query.trim();
        if query.is_empty() {
            self.scored = (0..self.items.len())
                .map(|item| Scored {
                    item,
                    positions: Vec::new(),
                })
                .collect();
            return;
        }

        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut ranked: Vec<(u32, usize, Vec<u32>)> = Vec::new();
        let mut positions: Vec<u32> = Vec::new();
        for (item, haystack) in self.haystacks.iter().enumerate() {
            positions.clear();
            if let Some(score) =
                pattern.indices(haystack.slice(..), &mut self.matcher, &mut positions)
            {
                positions.sort_unstable();
                positions.dedup();
                ranked.push((score, item, positions.clone()));
            }
        }
        // Best score first; equal scores keep their original newest-first order.
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.scored = ranked
            .into_iter()
            .map(|(_, item, positions)| Scored { item, positions })
            .collect();
    }

    /// Set the viewport height (e.g. after a terminal resize), keeping the
    /// selection visible.
    pub fn set_height(&mut self, height: usize) {
        self.height = height.max(1);
        self.scroll_to_selection();
    }

    /// Append a typed character to the query.
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.recompute();
    }

    /// Delete the character before the cursor (the query's last char).
    pub fn backspace(&mut self) {
        self.query.pop();
        self.recompute();
    }

    /// Delete the word before the cursor (readline `unix-word-rubout`): drop
    /// trailing whitespace, then the run of non-whitespace before it.
    pub fn delete_word(&mut self) {
        let trimmed = self.query.trim_end_matches(char::is_whitespace);
        let cut = trimmed
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        self.query.truncate(cut);
        self.recompute();
    }

    /// Clear the entire query.
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.recompute();
    }

    /// Move the selection toward the better-ranked end (index 0).
    ///
    /// In the bottom-anchored layout the best match is drawn at the bottom, so
    /// this is what the Down key / Ctrl-N maps to.
    pub fn select_better(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.scroll_to_selection();
        }
    }

    /// Move the selection toward the worse-ranked end (higher index).
    ///
    /// Worse matches are drawn higher up the screen, so this is what the Up key
    /// / Ctrl-P maps to.
    pub fn select_worse(&mut self) {
        if self.selected + 1 < self.scored.len() {
            self.selected += 1;
            self.scroll_to_selection();
        }
    }

    /// Adjust the scroll offset so the selected row sits within the viewport.
    fn scroll_to_selection(&mut self) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + self.height {
            self.offset = self.selected + 1 - self.height;
        }
    }

    /// The query text as currently typed.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// `(matching, total)` counts for the `m/n` indicator.
    pub fn counter(&self) -> (usize, usize) {
        (self.scored.len(), self.items.len())
    }

    /// The 1-based rank of the selection within the matches, or `None` when
    /// nothing matches. Drives the cursor-position part of the stats line.
    pub fn selected_rank(&self) -> Option<usize> {
        if self.scored.is_empty() {
            None
        } else {
            Some(self.selected + 1)
        }
    }

    /// The rows currently inside the viewport, top to bottom.
    pub fn visible(&self) -> Vec<VisibleRow<'_>> {
        let end = (self.offset + self.height).min(self.scored.len());
        (self.offset..end)
            .map(|i| {
                let s = &self.scored[i];
                VisibleRow {
                    matchable: &self.matchable[s.item],
                    suffix: &self.suffix[s.item],
                    positions: &s.positions,
                    selected: i == self.selected,
                }
            })
            .collect()
    }

    /// The list region as exactly `height` screen lines, top to bottom.
    ///
    /// The picker is bottom-anchored: the prompt sits at the bottom and the list
    /// grows upward with the best match nearest the prompt. So this returns the
    /// viewport rows in *screen* order (worst-ranked first/top, best-ranked
    /// last/bottom) and pads the top with `None` blanks when fewer rows match
    /// than the viewport can show, keeping the list flush above the prompt.
    pub fn list_lines(&self) -> Vec<Option<VisibleRow<'_>>> {
        let visible = self.visible();
        let blanks = self.height.saturating_sub(visible.len());
        let mut lines: Vec<Option<VisibleRow<'_>>> = Vec::with_capacity(self.height);
        lines.extend(std::iter::repeat_with(|| None).take(blanks));
        // `visible` is best-first; reverse it so the best match lands at the
        // bottom, nearest the prompt.
        lines.extend(visible.into_iter().rev().map(Some));
        lines
    }

    /// Consume the state and return the currently selected item, if any.
    pub fn into_selected(mut self) -> Option<T> {
        let item = self.scored.get(self.selected)?.item;
        // `swap_remove` is O(1) and order no longer matters once we are done.
        Some(self.items.swap_remove(item))
    }

    /// Render the visible state to a plain, deterministic string for snapshot
    /// tests, in screen order (top to bottom): each visible row prefixed with
    /// `> ` (selected) or two spaces and matched characters wrapped in `[ ]`,
    /// then the `m/n` counter line, then the prompt at the bottom.
    ///
    /// Rows come from [`Self::list_lines`], so they are bottom-anchored (best
    /// match nearest the prompt). The top blank-fill lines are omitted here to
    /// keep snapshots tight; blank-fill is covered directly in `list_lines`
    /// tests.
    #[cfg(test)]
    pub fn debug_render(&self) -> String {
        use std::collections::HashSet;
        use std::fmt::Write as _;

        // Trailing whitespace (e.g. the empty prompt's `> `) is trimmed per line
        // so inline snapshots stay free of fragile trailing spaces.
        let mut out = String::new();
        for row in self.list_lines().into_iter().flatten() {
            let pointer = if row.selected { "> " } else { "  " };
            let marked: HashSet<u32> = row.positions.iter().copied().collect();
            let mut line = String::new();
            for (i, c) in row.matchable.chars().enumerate() {
                if marked.contains(&(i as u32)) {
                    let _ = write!(line, "[{c}]");
                } else {
                    line.push(c);
                }
            }
            line.push_str(row.suffix);
            let _ = writeln!(out, "{}", format!("{pointer}{line}").trim_end());
        }
        let (m, n) = self.counter();
        let _ = writeln!(out, "{m}/{n}");
        let _ = writeln!(out, "{}", format!("> {}", self.query).trim_end());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed candidate set rendered by its own string identity (no suffix).
    fn state(rows: &[&str], height: usize) -> PickerState<String> {
        let items: Vec<String> = rows.iter().map(|s| s.to_string()).collect();
        let picker_rows = items
            .iter()
            .map(|s| Row {
                matchable: s.clone(),
                suffix: String::new(),
            })
            .collect();
        PickerState::new(items, picker_rows, height)
    }

    #[test]
    fn empty_query_lists_all_in_original_order() {
        let s = state(&["alpha", "beta", "gamma"], 10);
        // Bottom-anchored: best/selected row is nearest the prompt at the bottom.
        insta::assert_snapshot!(s.debug_render(), @r"
          gamma
          beta
        > alpha
        3/3
        >
        ");
    }

    #[test]
    fn query_filters_and_highlights_matches() {
        let mut s = state(&["alpha", "beta", "gamma"], 10);
        s.push_char('a');
        // Every row contains an 'a'; each keeps at least one highlighted match.
        insta::assert_snapshot!(s.debug_render());
    }

    #[test]
    fn non_matching_query_empties_the_list() {
        let mut s = state(&["alpha", "beta"], 10);
        for c in "zzz".chars() {
            s.push_char(c);
        }
        insta::assert_snapshot!(s.debug_render(), @r"
        0/2
        > zzz
        ");
    }

    #[test]
    fn selection_moves_and_clamps_at_both_ends() {
        let mut s = state(&["a", "b", "c"], 10);
        // Cannot move past the best-ranked end (index 0).
        s.select_better();
        assert_eq!(s.selected, 0);
        s.select_worse();
        s.select_worse();
        assert_eq!(s.selected, 2);
        // Cannot move past the worst-ranked end.
        s.select_worse();
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn viewport_scrolls_to_follow_the_selection() {
        let mut s = state(&["r0", "r1", "r2", "r3", "r4"], 2);
        // Only two rows fit; moving toward worse matches past the window scrolls
        // it. The selected (worse) row sits at the top, the better row below it.
        s.select_worse();
        s.select_worse();
        insta::assert_snapshot!(s.debug_render(), @r"
        > r2
          r1
        5/5
        >
        ");
        // Moving back toward the best match brings earlier rows into view again.
        s.select_better();
        s.select_better();
        insta::assert_snapshot!(s.debug_render(), @r"
          r1
        > r0
        5/5
        >
        ");
    }

    #[test]
    fn typing_resets_selection_to_the_top() {
        let mut s = state(&["alpha", "alps", "also"], 10);
        s.select_worse();
        s.select_worse();
        assert_eq!(s.selected, 2);
        s.push_char('a');
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn backspace_widens_the_result_set() {
        let mut s = state(&["alpha", "beta"], 10);
        s.push_char('b');
        assert_eq!(s.counter().0, 1);
        s.backspace();
        assert_eq!(s.counter().0, 2);
    }

    #[test]
    fn delete_word_drops_the_last_whitespace_run() {
        let mut s = state(&["x"], 10);
        for c in "foo bar".chars() {
            s.push_char(c);
        }
        s.delete_word();
        assert_eq!(s.query(), "foo ");
        s.delete_word();
        assert_eq!(s.query(), "");
    }

    #[test]
    fn clear_query_empties_the_input() {
        let mut s = state(&["x"], 10);
        for c in "abc".chars() {
            s.push_char(c);
        }
        s.clear_query();
        assert_eq!(s.query(), "");
    }

    #[test]
    fn into_selected_returns_the_cursor_row() {
        let mut s = state(&["alpha", "beta", "gamma"], 10);
        s.select_worse();
        assert_eq!(s.into_selected().as_deref(), Some("beta"));
    }

    #[test]
    fn into_selected_is_none_when_nothing_matches() {
        let mut s = state(&["alpha"], 10);
        s.push_char('z');
        assert_eq!(s.into_selected(), None);
    }

    #[test]
    fn suffix_is_shown_but_not_matched() {
        let items = vec!["alpha".to_string()];
        let rows = vec![Row {
            matchable: "alpha".to_string(),
            suffix: "  (ZID)".into(),
        }];
        let mut s = PickerState::new(items, rows, 10);
        // The suffix is part of the displayed row...
        insta::assert_snapshot!(s.debug_render(), @r"
        > alpha  (ZID)
        1/1
        >
        ");
        // ...but a character that occurs only in the suffix finds no match.
        s.push_char('z');
        assert_eq!(s.counter().0, 0);
    }

    #[test]
    fn empty_item_set_selects_nothing() {
        let s: PickerState<String> = PickerState::new(Vec::new(), Vec::new(), 10);
        assert_eq!(s.counter(), (0, 0));
        assert_eq!(s.into_selected(), None);
    }

    /// Collect `(matchable, selected)` for the non-blank lines, top to bottom.
    fn line_rows(s: &PickerState<String>) -> Vec<(String, bool)> {
        s.list_lines()
            .into_iter()
            .flatten()
            .map(|r| (r.matchable.to_string(), r.selected))
            .collect()
    }

    #[test]
    fn list_lines_pads_the_top_with_blanks() {
        let s = state(&["a", "b"], 5);
        let lines = s.list_lines();
        // Exactly `height` lines; the top is blank so the list hugs the prompt.
        assert_eq!(lines.len(), 5);
        assert!(lines[0].is_none());
        assert!(lines[1].is_none());
        assert!(lines[2].is_none());
        assert!(lines[3].is_some());
        assert!(lines[4].is_some());
    }

    #[test]
    fn list_lines_put_the_best_match_last_and_selected() {
        // Empty query keeps original order; index 0 ("a") is best and selected.
        let rows = line_rows(&state(&["a", "b", "c"], 10));
        assert_eq!(
            rows,
            vec![
                ("c".to_string(), false),
                ("b".to_string(), false),
                ("a".to_string(), true),
            ]
        );
    }

    #[test]
    fn list_lines_have_no_blanks_when_the_viewport_is_full() {
        let s = state(&["r0", "r1", "r2", "r3", "r4"], 2);
        let lines = s.list_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(Option::is_some));
    }

    #[test]
    fn selected_rank_is_one_based_and_tracks_movement() {
        let mut s = state(&["a", "b", "c"], 10);
        assert_eq!(s.selected_rank(), Some(1));
        s.select_worse();
        assert_eq!(s.selected_rank(), Some(2));
    }

    #[test]
    fn selected_rank_is_none_when_nothing_matches() {
        let mut s = state(&["alpha", "beta"], 10);
        s.push_char('z');
        assert_eq!(s.selected_rank(), None);
    }

    #[test]
    fn list_lines_track_the_selection_after_moving() {
        let mut s = state(&["a", "b", "c"], 10);
        s.select_worse();
        // The selected (now "b") row moves up one toward the worse end.
        let rows = line_rows(&s);
        assert_eq!(
            rows,
            vec![
                ("c".to_string(), false),
                ("b".to_string(), true),
                ("a".to_string(), false),
            ]
        );
    }
}
