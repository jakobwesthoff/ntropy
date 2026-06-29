# Non-interactive table column alignment

`print_notes` and `print_tags` in `src/bin/ntropy/run/output.rs` use raw `\t`
separators. When a value in one column is longer than the next tab stop (e.g. a
long tag name in `ntropy tags`, or a long title in `ntropy list`), the columns
misalign visually in terminals and pagers.

## What needs fixing

- `print_tags` (line 70): `tag<TAB>count` — a tag like
  `area/very/long/hierarchical/path` pushes the count column out of alignment.
- `print_notes` (line 26): `id<TAB>date<TAB>title<TAB>tags<TAB>path` — the
  title and tags fields are especially prone to overflow.

## Approach

Do a first pass over the data to compute the maximum width of each column,
then format every row (including the header) with `{:<width$}` padding, as
`print_info` already does for its "Top tags" section (lines 113–121 of
`output.rs`).

The separator between columns should stay human-friendly (two spaces is
conventional for aligned tables; tabs should be dropped for the padded path
since paths don't need alignment).

## Constraints

- ADR 0025 and the module-level doc comment describe the tab-separated format
  as intentionally machine-parseable (`awk`/`cut`, `tail -n +2`). Switching to
  space-padded output would break that contract.
- Resolution options to weigh:
  1. Keep tab output as-is (machine contract wins, misalignment accepted).
  2. Add a `--human` / `--table` flag that switches to aligned output.
  3. Auto-detect a TTY and emit aligned output only when stdout is a terminal
     (same pattern used for the interactive picker). Machine consumers always
     pipe, so they would never see the aligned format.
  4. Widen tabs to a computed width using ANSI padding (fragile, non-portable).

Option 3 is the most natural UX (same philosophy as the picker) but requires
checking whether the tab-format guarantee in ADR 0025 was meant to apply only
to piped output or also to TTY output. Review ADR 0025 before deciding.

## Related

- `src/bin/ntropy/run/output.rs` — `print_notes`, `print_tags`, `print_info`
- `docs/adr/` — ADR 0019, ADR 0025 (machine-friendly output contract)
