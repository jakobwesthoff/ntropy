# 66. Editor value run through the shell

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0015](0015-editor-integration-and-new-note-flow.md), which
resolves the editor from `$VISUAL` and `$EDITOR` but does not say how the
value is run, and the testing rule of
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md).

## Context

A `$VISUAL` or `$EDITOR` value often carries arguments, such as `code -w` or
`emacsclient -t`. ntropy took the whole value as the program's name, so a
value with arguments could not start the editor.

## Decision

ntropy runs the value through `/bin/sh`, the way git runs its editor:

    /bin/sh -c '<value> "$@"' '<value>' <note path>

The value is used verbatim, so quoting, `~` and `$VARIABLES` in it work as
they do in the shell. The note path is a positional parameter and never part
of the shell string, so no character in it is interpreted. Every value runs
through the shell, a single word included, so a program that cannot be found
is reported by the shell with exit status 127.

A value that is empty or only whitespace counts as unset. Run through the
shell, it would hand the note path to the shell as the command.

The rest of ADR 0015 holds: `$VISUAL` before `$EDITOR`, and an explicit error
when neither is set. The editor's stdin, stdout and stderr stay bound to the
controlling terminal (ADR 0036).

ADR 0036 has CLI contract tests pass `-n` or `--print` so that a local run
and CI take the same branch. A test that runs the binary on its own
pseudo-terminal, as its controlling terminal, is the second way: both
environments then take the interactive branch. The editor launch and its
terminal binding are tested that way.

## Rejected alternatives

- The `edit` crate: when the program in `$VISUAL`/`$EDITOR` is not found it
  moves on to other variables and a built-in list of editors, against ADR
  0015's explicit error, and it runs the editor with inherited stdin, stdout
  and stderr, against ADR 0036.
- Splitting the value on whitespace: leaves no way to name a program whose
  path contains spaces; through the shell such a path can be quoted.
- Splitting the value with `shell-words` without a shell: handles quoting but
  expands neither `~` nor `$VARIABLES`.

## Consequences

- A program path that contains spaces or other characters special to the
  shell needs quotes:
  `EDITOR="'/Applications/My Editor.app/Contents/MacOS/edit' -w"`.
- A program that cannot be found is reported by the shell on the terminal,
  and ntropy reports that the editor exited with a non-zero status.
- The value is shell code. One that ends in `#` or `\` keeps the note path
  from reaching the editor intact, and one that ends in `;`, `&`, `|` or a
  newline runs the editor without the note and then the note path as a
  command.
