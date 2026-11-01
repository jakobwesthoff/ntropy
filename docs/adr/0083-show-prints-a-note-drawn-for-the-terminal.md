# 83. `show` prints a note drawn for the terminal

Date: 2026-11-01

## Status

Accepted

Extends the command surface of [ADR 0018](0018-cli-command-surface.md), as
[ADR 0037](0037-render-command-surface.md) did. Interactivity follows
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md); the
drawing is the ansi engine of
[ADR 0082](0082-ansi-render-format-drawn-by-patine.md). Amends
[ADR 0070](0070-trust-model-for-discovered-vaults.md): the pager is a third
program ntropy starts.

## Context

`search -P` prints a note's source, and `render --to ansi` writes a drawn
note to a file. Reading a note formatted in the terminal needs a command
that writes the drawing there.

## Decision

    ntropy show [id|query] [--width <columns>]

- Selection follows `render` (ADR 0037): one note; several open the picker,
  or under `-n` are an error; no match, or an empty vault, exits 1; a
  cancelled picker does nothing.
- Width: `--width`, else the columns of the terminal stdout is, else 80.
- Styles: none when stdout is not a terminal; every style but the colours
  when `NO_COLOR` is set and not empty, as `NO_COLOR` asks for no colour
  and leaves bold, italic and underline alone; every style otherwise.
- Pager: when ntropy is interactive, stdout is a terminal and the drawing
  takes at least as many rows as the terminal has, the drawing goes to
  `$PAGER`, run through `/bin/sh` as the editor value is
  ([ADR 0066](0066-editor-value-run-through-the-shell.md)), a blank value
  counting as unset, or else to `less -R` from `PATH`. When `LESS` is unset,
  the pager gets `LESS=R`, the least that lets a bare `less` show the
  styles; a `LESS` of the user's own is left as it is. When `less` cannot
  be started, the drawing is printed. The drawing reaches the pager on a
  pipe, and a pager that stops reading is not an error. A `$PAGER` the shell
  could not run (status 126 or 127) is an error; every other exit status is
  ignored. While the pager runs, ntropy catches `SIGINT` with a handler that
  does nothing, and waits for the pager. It is a handler and not `SIG_IGN`
  because an ignored signal stays ignored in the pager.
- `-n` never pages.
- Scan warnings print to stderr. Under `--strict` the note is still shown
  and the exit code fails.
- An encrypted vault's note is drawn from memory, and ntropy writes nothing
  to disk. The pager gets `LESSSECURE=1`, which keeps less from saving,
  editing or piping what it shows. It constrains less alone: another
  `$PAGER` gets the plaintext as any pager would.
- The programs ntropy starts (ADR 0070) gain the pager, chosen by the
  environment alone: no vault value selects it or its arguments.

## Consequences

- A `$PAGER` that does not pass escape sequences through shows them as text.
