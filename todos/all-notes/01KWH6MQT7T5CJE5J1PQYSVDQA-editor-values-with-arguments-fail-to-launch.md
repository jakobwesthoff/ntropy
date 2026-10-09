---
title: "EDITOR values with arguments fail to launch"
kind: bug
component: cli
impact: high
origin: review
tags: [ux]
---
# EDITOR values with arguments fail to launch

Found in the 2026-07-02 codebase review of the CLI runtime. Values such as `EDITOR="code -w"` or `EDITOR="emacsclient -t"` are common, but ntropy treats the whole value as the program name and fails to launch the editor.

## Problem

Read in the code during the review. `editor::open` (`src/bin/ntropy/run/editor.rs`, around lines 18 to 32) passes the entire environment value as the program name:

```rust
Command::new(&editor).arg(path).status()
```

By long-standing Unix convention, `$VISUAL` and `$EDITOR` may contain a command with arguments. git, for example, runs the value through `sh -c '$EDITOR "$@"'`. With ntropy, `EDITOR="code -w"` tries to execute a program literally named `code -w` and fails with a spawn error ("while launching editor `code -w`"). ADR 0015 specifies the `$VISUAL` and `$EDITOR` resolution but does not address argument handling.

## Impact

Users whose editor setting carries arguments cannot open notes in their editor from ntropy.

## Suggested fix

Two options:

1. Run the value through the shell, as git does: `sh -c "$EDITOR \"$1\"" -- <path>`. The value is used verbatim, and the note path is passed as a positional argument so it is never injected into the string.
2. Split the value on whitespace, treat the first token as the program and the rest as leading arguments. This needs no shell, but breaks paths with spaces, such as `/Applications/My Editor.app/...`, which option 1 handles when quoted.

Option 1 matches what git and crontab users expect and handles quoting. Prefer it unless a no-shell policy applies.

## Open questions

Is a no-shell policy wanted? If so, option 2 applies, and the limitation for paths with spaces stays.

## Done when

- A multi-token value launches the editor, and the file argument arrives. An integration test can assert this with a small script or a `sh`-friendly command.
- Single-token values keep working unchanged.
