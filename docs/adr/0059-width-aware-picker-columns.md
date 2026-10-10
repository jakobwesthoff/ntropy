# 59. Width-aware picker columns

Date: 2026-10-10

## Status

Accepted

Amends [ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md),
whose column widths came from absolute caps (title 48, tags 32 columns) and
whose rows ended in the note's ULID.

## Context

The caps truncated titles and tag lists with an ellipsis on terminals wide
enough to show them in full. A full row was 128 columns wide. On narrower
terminals the draw loop cut each row at the right edge, removing the ULID
first and then the tags.

## Decision

The picker splits the terminal width between the title and tag columns and
lays every row out again when the width changes.

- `render_all` receives the width available for a row and is called again
  when the terminal width changes.
- Widths are measured over all candidates, not over the current matches.
- The space left after the date and the separators is split 70/30 between
  the title and the tags. A column that needs less than its share takes only
  what it needs and leaves the rest to the other. A title or tag list is cut
  with an ellipsis only when its column is narrower than it.
- The title column has no fixed upper limit.
- The tag list is the last column and is not padded.
- The ULID is no longer part of a row. The stats line under the prompt shows
  the selected note's ULID.
- A width change keeps the ranking, the selection and the scroll position,
  and recomputes the match highlights against the new rows.

## Consequences

- Column positions change when the terminal width changes. ADR 0027's grid
  did not.
- Only the selected note's ULID is on screen.
