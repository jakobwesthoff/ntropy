# Picker: fuzzy search must cover full content, not just displayed text

## Problem

The fuzzy matcher searches only what is visible in the picker columns.
In `layout.rs`, `align_candidates` pre-truncates titles (cap 48 cols)
and tag lists (cap 32 cols) before joining them into the `matchable`
string. Those truncated strings are what gets fed into nucleo as
`haystacks`. Any content beyond the caps — tags like `rust, cli, …` where
the third tag was cut — is completely invisible to the search.

## Root cause

`align_candidates` (`layout.rs`) builds the row's matchable text from
already-truncated strings. `PickerState::new` (`state.rs`, ~line 80)
converts those matchable strings directly into `Utf32String` haystacks
for nucleo. There is no separate "full-content" search string.

## Desired behaviour

Display columns remain width-capped and show `…` where content is
clipped. Fuzzy search, however, must match against the full untruncated
content: the complete title and the complete tag list.

## Suggested approach

Introduce a parallel `search_text: Vec<String>` in `PickerState` (or
an equivalent field on `Row`) built from the full, untruncated
candidate data. Use this for `haystacks` instead of the display
`matchable` string.

Highlight rendering is the tricky part: nucleo returns byte/char
indices into the haystack, but the terminal renders the truncated
`matchable`. Options:

1. **Offset mapping**: build a mapping from haystack char positions to
   display positions. Positions that fall inside truncated-away content
   have no display counterpart and are simply not highlighted. Positions
   within the visible portion map 1-to-1 (titles share the same prefix;
   tags share the same prefix up to the ellipsis).
2. **Full text in matchable, truncated only for display**: keep a
   separate `display` string alongside `matchable`, render `display`
   but run nucleo over `matchable`. Highlight indices then need the
   same offset translation.

Option 1 is simpler because the visible prefix of a truncated field is
identical to the same prefix of the full field — only the tail differs.
Any match index ≤ cap length maps directly; any index beyond is dropped.

## Files to touch

- `src/bin/ntropy/run/picker/layout.rs` — expose full (untruncated)
  title and tag strings alongside the display strings from
  `align_candidates`.
- `src/bin/ntropy/run/picker/state.rs` — build `haystacks` from full
  strings; translate highlight indices back to display positions before
  storing in `Scored`.
- `src/bin/ntropy/run/picker/mod.rs` — rendering loop consumes
  translated indices; no change expected beyond signature adjustments.
