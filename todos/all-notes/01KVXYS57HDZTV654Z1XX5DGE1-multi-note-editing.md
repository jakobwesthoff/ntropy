# Multi-note editing (post-v1)

Deferred from v1 per ADR 0015. v1 edits a single note at a time and the
interactive picker is single-select.

## To decide later

- Enable multi-select in the picker.
- Open all selected notes in one editor invocation (e.g. `nvim file1 file2 …`).
- Reconcile all touched notes on the single editor exit.
