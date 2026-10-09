---
title: "Add color to decorated terminal output"
kind: improvement
component: cli
origin: discussion
tags: [ux]
---
# Add color to decorated terminal output

Deferred during the v1 design (ADR 0024). v1 TTY output is plain, without
color.

## Proposal

- Add color to decorated TTY output.
- Respect `NO_COLOR` and TTY detection, and add a `--color=auto|always|never`
  flag.

## Open questions

- Which library should provide the styling? `anstyle` with `anstream` aligns
  with clap. `owo-colors` is the alternative.
