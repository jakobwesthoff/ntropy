---
title: "Show the exit status when the editor fails"
kind: improvement
component: cli
impact: low
origin: implementation
tags: [error-handling]
---
# Show the exit status when the editor fails

Found on 2026-10-30 while running `$VISUAL`/`$EDITOR` through `/bin/sh`
(ADR 0066). When the editor exits unsuccessfully, ntropy prints
"editor `X` exited with a non-zero status" and drops the status itself,
so the person cannot tell an editor that quit with `:cq` from a command
the shell could not find (status 127) or an editor killed by a signal.

## Goal

The message names the status, for example
"editor `X` exited with status 127" or "editor `X` was terminated by
signal 9".

## Proposal

`src/bin/ntropy/run/editor.rs`, `open`, has the `ExitStatus` at hand
when it bails. Format it from `ExitStatus::code()` and, on Unix,
`std::os::unix::process::ExitStatusExt::signal()`. On macOS
(checked 2026-10-30) `/bin/sh -c` execs the editor, so a killed editor
reports the signal itself; a shell that forks instead exits with 128
plus the signal number. The CLI test in
`tests/cli.rs` that asserts "exited with a non-zero status" for
`no-such-editor --wait` changes with it.

## Open questions

- Whether status 127 gets a hint that the shell did not find the
  command. The shell's own "not found" line already reaches the
  terminal, since the editor's stderr is bound to it.
