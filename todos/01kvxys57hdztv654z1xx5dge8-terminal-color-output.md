# Terminal color output (post-v1)

Deferred from v1 per ADR 0024. v1 TTY output is plain (no color).

## To decide later

- Add coloring for decorated TTY output (`anstyle`/`anstream` aligns with clap;
  `owo-colors` is an alternative).
- Respect `NO_COLOR` and TTY detection; a `--color=auto|always|never` flag.
