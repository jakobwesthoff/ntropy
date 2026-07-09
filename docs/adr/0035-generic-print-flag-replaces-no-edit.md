# 35. Generic --print flag replaces --no-edit

Date: 2026-07-09

## Status

Accepted

Supersedes the `--no-edit` flag naming of
[ADR 0015](0015-editor-integration-and-new-note-flow.md) and
[ADR 0018](0018-cli-command-surface.md).

The non-interactive `--print` behavior is superseded by
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md): without
a picker, `--print` prints every match one path per line instead of leaving
the table unchanged, and interactivity itself keys off the controlling
terminal rather than stdout.

## Context

`new` and `today` suppressed the editor with `--no-edit`, which carried
`--print` as a visible alias and had no short form. `search` had no way to
suppress the editor at all: on a TTY a match always opened, so a script could
not obtain the selected note's path.

## Decision

One flag, `--print` with the short form `-p`, on every command that would
open the editor — a rule that also binds future editor-opening commands.
`--print` is preferred over `--no-edit` because it states what the command
does instead of negating what it skips. `--no-edit` remains a hidden alias
for backward compatibility; help output documents only `--print`/`-p`.

On `search` (aliases `list` and `edit`), `--print` writes the selected note's
path to stdout instead of opening the editor:

- On a TTY a lone match prints directly; several matches open the picker and
  the selection's path prints. The path prints as-is, exactly as `new` and
  `today` print it.
- A cancelled picker exits non-zero, so `p=$(ntropy search -p ...)` branches
  correctly; without `--print` a cancel stays a successful no-op.
- Without a TTY the flag changes nothing: the plain table prints as before
  (ADR 0031), and the editor never opens anyway (ADR 0015).

## Consequences

- The flag surface is uniform: `new`, `today` and `search` all take
  `--print`/`-p`, and `--no-edit` parses everywhere it did before, plus on
  `search`.
- Existing scripts using `--no-edit` keep working unchanged.
- `search --print` gives scripts a path-only handle on a note chosen through
  the interactive picker, which no command offered before.
