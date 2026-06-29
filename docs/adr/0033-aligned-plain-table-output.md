# 33. Aligned plain-table output

Date: 2026-06-29

## Status

Accepted

Supersedes the plain-table-format decision of
[ADR 0025](0025-plain-output-format-ordering-and-edit-ambiguity.md). The
ordering decision (newest first) and the edit/delete ambiguity rules of ADR 0025
still stand.

## Context

ADR 0025 made the non-interactive tables tab-separated so `awk -F'\t'` and
`cut -f` could split them positionally and `tail -n +2` could drop the header.
That format reads poorly in a terminal: a tab advances to the next tab stop, so
the moment one cell is wider than that stop the following columns step out of
line, and the stop width is a viewer setting (commonly 8, but `less -x`, pagers
and editors vary), so the misalignment is not even consistent between viewers.

The two goals are in genuine tension. Positional parsing needs a delimiter the
data cannot contain; tabs give that, but only at the cost of the tab-stop
rendering above. Visual alignment needs variable padding the parser must ignore;
spaces give that, but a note title routinely contains spaces, so once columns
are space-padded `awk '{print $3}'` no longer isolates the title and the field
count drifts row to row. Padding with repeated tabs is worse than either: the
empty fields it injects make a column's field index depend on the data, breaking
the positional contract outright, and it still only lines up under one assumed
tab width. No single text encoding is both a clean visual table and a clean
positional-field stream.

On a TTY the interactive picker already covers note browsing, but `-n`, piped
output, and redirected output all surface the raw table, so the format still has
to serve a human reading it directly.

## Decision

All plain tables render space-aligned for every invocation, whether on a TTY,
piped, or forced plain with `-n`: the note table (`search`/`list`), the tag
table (`tags`), and the view table (`view list`). Each column is padded to the
widest cell in Unicode display width and separated by two spaces. The final
column is never padded, so no line carries trailing whitespace. The uppercase
header row stays, so the output is self-describing and `tail -n +2` still strips
it.

The tab-separated positional contract is retired: `awk -F'\t'` and `cut -f` no
longer split the tables. Machine consumers are served by a structured (JSON)
output mode, deferred to its own decision and tracked separately, rather than by
positional text.

## Consequences

- The same aligned table reaches every consumer, so a human reading piped or
  `-n` output sees lined-up columns without the tab-stop drift.
- Scripts that split the old format with `cut -f` or `awk -F'\t'` break. Until
  the structured output mode lands there is no positional parse contract; such
  scripts must tolerate the aligned columns (for example by field position with
  whitespace-collapsing tools, accepting that whitespace inside a value is
  ambiguous) or wait for JSON.
- `tail -n +2` still drops the header, and the newest-first ordering of ADR 0025
  is unchanged.
- Column widths are computed in Unicode display width, so wide (CJK) and
  zero-width characters in titles and tags align correctly, which the earlier
  byte-length padding in the `info` report did not guarantee.
