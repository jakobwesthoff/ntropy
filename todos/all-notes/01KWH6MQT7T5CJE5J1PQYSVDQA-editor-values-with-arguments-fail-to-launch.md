---
title: "EDITOR values with arguments fail to launch"
kind: bug
component: cli
impact: critical
horizon: next
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

## Priority

Raised to critical and queued next by the user on 2026-10-28, while the
same question came up in asqr (a `ctrl-g` that opens a field in the
person's editor): an editor setting with arguments is common, and ntropy
cannot open notes at all for those users.

## Research from asqr, 2026-10-28

Checked in the source of the `edit` crate 0.1.5, which yaw uses
(`yaw/src/editor.rs` calls `edit::edit_file`) and asqr adopts for
`ctrl-g`:

- It reads `VISUAL`, then `EDITOR`, and splits the value into program
  and arguments: on whitespace by default, with `shell_words::split`
  when the `quoted-env` feature is on, falling back to whitespace when
  the value does not parse. So `code -w` and, with `quoted-env`,
  `"/Applications/My Editor.app/…/bin/edit" -w` both work.
- It looks the program up in `PATH` (`which`, default feature
  `better-path`). If the program from `VISUAL`/`EDITOR` is not found, it
  silently moves on to the next variable and then to a hard-coded list
  of common editors and openers. That contradicts ADR 0015's explicit
  error when neither variable is set.
- `edit_file` runs the editor with inherited stdin, stdout and stderr.
  ntropy binds all three to the controlling terminal instead (ADR 0036),
  so a pipeline such as `search -p | pbcopy` does not hand the editor a
  redirected stdout. `edit_file` cannot do that.
- The function that returns program and arguments (`get_editor_args`)
  is private; the public `get_editor` returns the program alone and
  drops the arguments.

So the `edit` crate does not fit ntropy as it is: it would drop the
terminal binding and the explicit error. What it does for splitting is
what ntropy needs, and that part is the `shell-words` crate on its own.

### A third option

3. Split the value with `shell_words::split` (no shell involved): the
   first word is the program, the rest are leading arguments, then the
   note path. It handles quoting, so paths with spaces work when quoted,
   which removes option 2's limitation without running a shell, and it
   keeps `build_command`, the terminal binding and the explicit error
   unchanged. A value that does not parse (an unclosed quote) becomes an
   error naming the variable.

Option 3 answers the open question below whichever way it goes: it
needs no shell, and it handles quoted paths.

## Open questions

Is a no-shell policy wanted? If so, option 2 applies, and the limitation for paths with spaces stays.

## Done when

- A multi-token value launches the editor, and the file argument arrives. An integration test can assert this with a small script or a `sh`-friendly command.
- Single-token values keep working unchanged.
