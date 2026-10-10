# 59. Width-aware picker columns

Date: 2026-10-10

## Status

Accepted

Amends [ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md),
whose column widths came from absolute caps (title 48, tags 32 columns).

## Context

The caps truncated titles and tag lists with an ellipsis on terminals wide
enough to show them in full. A full row was 128 columns wide. On narrower
terminals the draw loop cut each row at the right edge, removing the ULID
first and then the tags.

## Decision

The picker sizes the title and tag columns from the terminal width and
rebuilds every row when the width changes.

- `render_all` receives the width available for a row and is called again
  when the terminal width changes.
- Widths are measured over all candidates, not over the current matches.
- The tag column first gets a third of the space left after the date and the
  separators, or 12 columns if that is more, but never more than its widest
  tag list.
- The title then takes up to 80 columns of the rest. If that leaves the title
  fewer than 24 columns (or fewer than its widest title, if shorter), the
  title takes up to that many from the tags, as far as the space allows.
- The tags then get the space the title leaves, up to their widest tag list;
  the tag column has no fixed cap.
- The ULID is shown only when the space left after the title (up to its
  80-column limit) and the tags (at their full width) holds it.
- A width change keeps the ranking, the selection and the scroll position,
  and recomputes the match highlights against the new rows.

## Consequences

- Column positions change when the terminal width changes. ADR 0027's grid
  did not.
- On a narrow terminal the tag column can get fewer than 12 columns, or none.
- One long tag list widens the tag column for every row and keeps the ULID
  hidden until the terminal fits it.
