# Picker stats line: Tier 2 (content-derived) stats

The picker's dimmed stats line under the prompt currently shows Tier 1
(index-only) data: cursor rank, match count, total count, and an empty-state
hint (`src/bin/ntropy/run/picker/mod.rs`, `stats_line`). These come straight
from `PickerState`, which is generic over `T` and knows only indices, the
query, and the matched set.

Tier 2 stats would surface information derived from the matched **notes**
themselves:

- Distinct tag count across the current matches (e.g. `8 tags`).
- Date span of the matches (e.g. `2024-01 → 2026-06`).
- Top / most-common tag among the matches (e.g. `top: work`).

## Why deferred

These need `Candidate` fields (title/date/tags), which the generic
`PickerState<T>` deliberately does not know (ADR 0027). Adding them requires a
design decision on how to feed content into the stats line without breaking the
generic picker. Leading option discussed:

- Pass a stats callback `Fn(&[&T]) -> String` into `pick`, invoked on the
  current matched subset each keystroke. Keeps `PickerState` generic; the binary
  computes the `Candidate`-specific summary. Cost: per-keystroke recomputation
  over the matched set, plus deciding how to compose/truncate the extra segments
  on the (already dimmed, width-limited) line.

Alternatives to weigh: make the picker `Candidate`-specific (drops the generic
abstraction), or precompute a summary that ignores the live query (cheaper but
less useful since it would not reflect the current filter).

## Open questions for the future discussion

- Which Tier 2 stats are actually worth the plumbing?
- Recompute on every keystroke vs. debounce vs. only on selection change?
- Layout: how to compose multiple segments with the Tier 1 stats and keep the
  line readable under narrow terminals (separator, ordering, truncation
  priority)?
