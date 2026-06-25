# 14. Interactive-by-default CLI with auto output mode

Date: 2026-06-24

## Status

Accepted

## Context

ntropy must serve both interactive use (fuzzy-pick a note, open it) and
scripting (clean output to pipe). The trigger and the output format need
defined behavior on a TTY versus a pipe.

## Decision

Interactive is the default on a TTY: listing/searching launches an in-process
fuzzy picker, and selecting a note opens it in `$EDITOR`. When stdout is not a
TTY (piped), commands are non-interactive automatically. `--non-interactive`
/ `-n` forces non-interactive on a TTY.

The picker is embedded over `nucleo` (matcher); its UI is rendered in-house
([ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md)). No
shelling out to `fzf`.

Output format auto-adapts: decorated/aligned for a TTY, plain machine-friendly
lines when piped. JSON output is out of scope for v1.

## Consequences

- The common case (open a terminal, find a note, edit it) needs no flags.
- Piping works without flags; plain output is the non-TTY default.
- No JSON in v1, so structured machine consumption is limited to the plain
  line format until JSON is added.
- ntropy owns picker UI behavior in-house rather than reusing a user's `fzf`
  configuration.
