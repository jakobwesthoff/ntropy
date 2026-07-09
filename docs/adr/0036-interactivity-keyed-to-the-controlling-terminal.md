# 36. Interactivity keyed to the controlling terminal

Date: 2026-07-09

## Status

Accepted

Supersedes the stdout-based interactivity detection and auto output mode of
[ADR 0014](0014-interactive-by-default-cli-with-auto-output-mode.md) and the
non-interactive `--print` behavior of
[ADR 0035](0035-generic-print-flag-replaces-no-edit.md).

## Context

ADR 0014 keyed interactivity off stdout being a terminal, using one signal
for two different questions: whether a human can interact, and whether stdout
is a display. The two diverge exactly in the select-then-pipe composition:
in `ntropy search -p | pbcopy` or `p=$(ntropy search -p ...)`, stdout is a
pipe while a human sits at the keyboard, so the picker never opened and
`--print` could not deliver its path to a script. Two implementation details
reinforced the coupling: the picker drew its UI on stdout, and the editor
inherited ntropy's file descriptors, so neither could share stdout with
piped data.

## Decision

ntropy is interactive if and only if a controlling terminal (`/dev/tty`) can
be opened and `-n` was not passed. Redirecting stdout or stdin does not
demote to plain mode. stdout is purely a data channel; all human interaction
goes through the controlling terminal:

- the picker renders to and reads keys from the controlling terminal,
- `delete`'s confirmation writes its prompt to and reads its answer from the
  controlling terminal,
- the editor is spawned with stdin, stdout, and stderr bound to the
  controlling terminal, so a full-screen editor works while ntropy's own
  stdout feeds a pipe.

`--print` composes with this: interactively the selected note's path is the
only thing written to stdout; non-interactively every match prints as one
path per line, newest first, since no picker can narrow the result.
Interactive `search` without `--print` writes nothing to stdout; the editor
session is the outcome.

Where no controlling terminal exists (cron, CI, `docker exec` without `-t`),
plain mode engages automatically, so nothing blocks waiting for input.

This breaks two former behaviors, accepted deliberately: `ntropy search ...
| grep` no longer prints the table into the pipe without `-n`, and
`path=$(ntropy new Title)` without `--print` (or `-n`) opens the editor
instead of printing the path.

## Consequences

- `ntropy search -p | pbcopy` opens the picker on the terminal and pipes only
  the selected path; a cancelled picker exits non-zero, so command
  substitution branches correctly.
- Scripts must state their intent: `-n` for tables, `-p` for paths. The agent
  skill has always mandated `-n`, so the documented scripting path is
  unchanged.
- CLI contract tests must pass `-n` or `--print` on every invocation that
  branches on interactivity: a local `cargo test` has a controlling terminal
  while CI may not, and only those flags make both environments take the
  same branch (extending ADR 0021's non-interactive testing boundary).
- The interactive plumbing (picker, prompt, editor fd binding) is validated
  manually, driven through a pseudo-terminal with stdout redirected
  (ADR 0021).
