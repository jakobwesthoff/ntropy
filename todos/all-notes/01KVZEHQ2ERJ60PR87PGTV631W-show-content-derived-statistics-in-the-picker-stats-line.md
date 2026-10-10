---
title: "Show content-derived statistics in the picker stats line"
kind: improvement
component: cli
status: needs-discussion
tags: [ux]
---
# Show content-derived statistics in the picker stats line

The picker's dimmed stats line under the prompt shows index-derived data
(cursor rank, match count, total count, and an empty-state hint) followed by
the selected note's ULID (`stats_line` in `src/bin/ntropy/run/picker/mod.rs`,
ADR 0059). The counts come from `PickerState`, which is generic over `T` and
knows only indices, the query and the matched set. The ULID reaches it as the
selected row's `Row.detail`, a per-row string the renderer fills; on a narrow
terminal the line is cut from the end, so the ULID goes first.

## Goal

Tier 2 stats would surface information derived from the matched notes
themselves:

- The distinct tag count across the current matches, such as `8 tags`.
- The date span of the matches, such as `2024-01 to 2026-06`.
- The most common tag among the matches, such as `top: work`.

## Why deferred

These need `Candidate` fields (title, date and tags), which the generic
`PickerState<T>` does not know by design (ADR 0027). Adding them requires a
design decision on how content reaches the stats line without breaking the
generic picker.

The leading option is to pass a stats callback `Fn(&[&T]) -> String` into
`pick`, invoked on the current matched subset on each keystroke. This keeps
`PickerState` generic, and the binary computes the `Candidate`-specific
summary. The cost is per-keystroke recomputation over the matched set, plus
deciding how to compose and truncate the extra segments on the dimmed,
width-limited line.

The alternatives are to make the picker `Candidate`-specific, which drops the
generic abstraction, or to precompute a summary that ignores the live query.
The precomputed summary is cheaper but less useful, because it does not
reflect the current filter.

## Open questions

- Which tier 2 stats are worth the plumbing?
- Should stats recompute on every keystroke, be debounced, or update only on
  selection change?
- How are the extra segments composed with the tier 1 stats, and how does the
  line stay readable on narrow terminals? Consider the separator, the ordering
  and the truncation priority.
