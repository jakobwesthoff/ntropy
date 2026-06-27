# Introduce a force-interactive flag

## Status

Open.

## Context

ntropy chooses interactive vs. plain mode from the TTY and the
`-n`/`--non-interactive` flag (`interact::is_interactive`, dispatched in
`src/bin/ntropy/run/mod.rs`). There is a way to force plain mode (`-n`) but no
way to force interactive mode. When stdout is not a TTY, the interactive paths
are unreachable, even with `$EDITOR` set.

Concretely, the editor-open-and-refresh path in `cmd_search` (the `edit` alias,
ADR 0031) only runs under `if interactive` (`mod.rs:170`). The benchmark harness
(`scripts/benchmark.sh`) pipes stdout, so it cannot exercise that path; its
`edit-open` row only measured a resolve-and-print and was removed for being
misleading. A force-interactive flag would let the harness (and scripted/CI
flows) drive the real resolve → open `$EDITOR` (stubbed) → realign → view-sync
cycle.

## Task

Add a flag that forces interactive mode regardless of TTY detection — the
counterpart to `-n`/`--non-interactive`. Decide the surface and precedence:

- Flag name (e.g. `--interactive`/`-i`) and whether it pairs cleanly with the
  existing `-n` on the global args.
- Precedence when both `--interactive` and `--non-interactive` are given (error,
  or last-wins).
- How `is_interactive` should combine the forced-on flag, forced-off flag, and
  TTY detection.
- Behavior of editor-spawning paths when forced interactive without a real TTY
  (relies on `$EDITOR`; the picker may misbehave without a terminal — decide
  whether the flag implies "single-match opens, multi-match errors" or similar).

## Follow-up once it exists

- Restore an `edit` benchmark in `scripts/benchmark.sh` that forces interactive
  with `EDITOR=true` to measure the full resolve-open-and-sync cycle (the
  motivation for removing the old, misleading `edit-open` row).

## References

- `src/bin/ntropy/run/mod.rs` — `is_interactive` use and the `cmd_search`
  interactive gate (`mod.rs:170`).
- ADR 0014 (interactive-by-default CLI with auto output mode).
- ADR 0031 (merge edit into search).
