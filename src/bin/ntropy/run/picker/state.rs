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
    /// The full rendered display text of the row.
    pub text: &'a str,
    /// Char positions in `text` that matched the query, ascending.
    pub positions: &'a [u32],
    /// Whether this row is the current selection.
    pub selected: bool,
}

/// The picker's interaction state over an owned set of items of type `T`.
pub struct PickerState<T> {
    /// The candidate items, index-aligned with `rows` and `haystacks`.
    items: Vec<T>,
    /// Pre-rendered display string per item.
    rows: Vec<String>,
    /// Pre-converted match haystacks per item (the same text as `rows`).
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
    /// Build the state from `items`, rendering each via `render`.
    ///
    /// `height` is the number of list rows the viewport can show; it is clamped
    /// to at least one so movement and scrolling always have room.
    pub fn new(items: Vec<T>, render: impl Fn(&T) -> String, height: usize) -> Self {
        let rows: Vec<String> = items.iter().map(&render).collect();
        let haystacks: Vec<Utf32String> =
            rows.iter().map(|r| Utf32String::from(r.as_str())).collect();
        let mut state = Self {
            items,
            rows,
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

    /// Move the selection one row toward the top.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.scroll_to_selection();
        }
    }

    /// Move the selection one row toward the bottom.
    pub fn move_down(&mut self) {
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

    /// The rows currently inside the viewport, top to bottom.
    pub fn visible(&self) -> Vec<VisibleRow<'_>> {
        let end = (self.offset + self.height).min(self.scored.len());
        (self.offset..end)
            .map(|i| {
                let s = &self.scored[i];
                VisibleRow {
                    text: &self.rows[s.item],
                    positions: &s.positions,
                    selected: i == self.selected,
                }
            })
            .collect()
    }

    /// Consume the state and return the currently selected item, if any.
    pub fn into_selected(mut self) -> Option<T> {
        let item = self.scored.get(self.selected)?.item;
        // `swap_remove` is O(1) and order no longer matters once we are done.
        Some(self.items.swap_remove(item))
    }

    /// Render the visible state to a plain, deterministic string for snapshot
    /// tests: the prompt, the `m/n` counter, then each visible row prefixed with
    /// `> ` (selected) or two spaces, with matched characters wrapped in `[ ]`.
    #[cfg(test)]
    pub fn debug_render(&self) -> String {
        use std::collections::HashSet;
        use std::fmt::Write as _;

        // Trailing whitespace (e.g. the empty prompt's `> `) is trimmed per line
        // so inline snapshots stay free of fragile trailing spaces.
        let mut out = String::new();
        let _ = writeln!(out, "{}", format!("> {}", self.query).trim_end());
        let (m, n) = self.counter();
        let _ = writeln!(out, "{m}/{n}");
        for row in self.visible() {
            let pointer = if row.selected { "> " } else { "  " };
            let marked: HashSet<u32> = row.positions.iter().copied().collect();
            let mut line = String::new();
            for (i, c) in row.text.chars().enumerate() {
                if marked.contains(&(i as u32)) {
                    let _ = write!(line, "[{c}]");
                } else {
                    line.push(c);
                }
            }
            let _ = writeln!(out, "{}", format!("{pointer}{line}").trim_end());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed candidate set rendered by its own string identity.
    fn state(rows: &[&str], height: usize) -> PickerState<String> {
        let items: Vec<String> = rows.iter().map(|s| s.to_string()).collect();
        PickerState::new(items, |s: &String| s.clone(), height)
    }

    #[test]
    fn empty_query_lists_all_in_original_order() {
        let s = state(&["alpha", "beta", "gamma"], 10);
        insta::assert_snapshot!(s.debug_render(), @r"
        >
        3/3
        > alpha
          beta
          gamma
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
        > zzz
        0/2
        ");
    }

    #[test]
    fn selection_moves_and_clamps_at_both_ends() {
        let mut s = state(&["a", "b", "c"], 10);
        // Cannot move above the top.
        s.move_up();
        assert_eq!(s.selected, 0);
        s.move_down();
        s.move_down();
        assert_eq!(s.selected, 2);
        // Cannot move past the bottom.
        s.move_down();
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn viewport_scrolls_to_follow_the_selection() {
        let mut s = state(&["r0", "r1", "r2", "r3", "r4"], 2);
        // Only two rows fit; moving down past the window scrolls it.
        s.move_down();
        s.move_down();
        insta::assert_snapshot!(s.debug_render(), @r"
        >
        5/5
          r1
        > r2
        ");
        // Scrolling back up brings earlier rows into view again.
        s.move_up();
        s.move_up();
        insta::assert_snapshot!(s.debug_render(), @r"
        >
        5/5
        > r0
          r1
        ");
    }

    #[test]
    fn typing_resets_selection_to_the_top() {
        let mut s = state(&["alpha", "alps", "also"], 10);
        s.move_down();
        s.move_down();
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
        s.move_down();
        assert_eq!(s.into_selected().as_deref(), Some("beta"));
    }

    #[test]
    fn into_selected_is_none_when_nothing_matches() {
        let mut s = state(&["alpha"], 10);
        s.push_char('z');
        assert_eq!(s.into_selected(), None);
    }

    #[test]
    fn empty_item_set_selects_nothing() {
        let s: PickerState<String> = PickerState::new(Vec::new(), |s: &String| s.clone(), 10);
        assert_eq!(s.counter(), (0, 0));
        assert_eq!(s.into_selected(), None);
    }
}
