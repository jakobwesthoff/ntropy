---
title: "Add a flag that forces interactive mode"
kind: feature
component: cli
origin: implementation
---
# Add a flag that forces interactive mode

ntropy picks interactive or plain mode from the TTY and the
`-n`/`--non-interactive` flag (`interact::is_interactive`, dispatched in
`src/bin/ntropy/run/mod.rs`). There is a way to force plain mode but none to
force interactive mode. When stdout is not a TTY, the interactive paths are
unreachable, even with `$EDITOR` set.

## Context

The editor-open-and-refresh path in `cmd_search` (the `edit` alias, ADR 0031)
runs only under the interactive gate, around line 170 of
`src/bin/ntropy/run/mod.rs`. The benchmark harness (`scripts/benchmark.sh`)
pipes stdout, so it cannot exercise that path. Its `edit-open` row measured
only a resolve-and-print, and it was removed for being misleading. A
force-interactive flag would let the harness, and scripted or CI flows, drive
the real cycle: resolve, open `$EDITOR` (stubbed), realign, and view sync.

## Proposal

Add a flag that forces interactive mode regardless of TTY detection, the
counterpart to `-n`/`--non-interactive`.

## Open questions

- What should the flag be called, for example `--interactive` or `-i`, and does
  it pair cleanly with the existing `-n` on the global args?
- When both `--interactive` and `--non-interactive` are given, is that an error,
  or does the last one win?
- How does `is_interactive` combine the forced-on flag, the forced-off flag and
  TTY detection?
- How do editor-spawning paths behave when forced interactive without a real
  TTY? They rely on `$EDITOR`, and the picker may misbehave without a terminal.
  Should the flag imply that a single match opens and multiple matches error, or
  something similar?

## Follow-up

Once the flag exists, restore an `edit` benchmark in `scripts/benchmark.sh` that
forces interactive mode with `EDITOR=true`, measuring the full resolve, open and
sync cycle. That benchmark is the motivation for removing the old `edit-open`
row.

## References

- ADR 0014 (interactive-by-default CLI with auto output mode).
- ADR 0031 (merge edit into search).
