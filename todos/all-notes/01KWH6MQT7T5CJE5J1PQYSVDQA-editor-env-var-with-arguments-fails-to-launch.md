# `$VISUAL`/`$EDITOR` values containing arguments fail to launch

Found during the 2026-07-02 codebase review (unit 10, CLI runtime).

## Problem

`editor::open` (`src/bin/ntropy/run/editor.rs:18-32`) passes the entire
env-var value as the program name:

```rust
Command::new(&editor).arg(path).status()
```

By long-standing Unix convention, `$VISUAL`/`$EDITOR` may contain a command
*with arguments* — `EDITOR="code -w"`, `EDITOR="emacsclient -t"`,
`EDITOR="vim -u ~/.vimrc-notes"` are all common. git, for example, runs the
value through `sh -c '$EDITOR "$@"'`. With ntropy, `EDITOR="code -w"` tries
to exec a program literally named `code -w` and fails with a spawn error
("while launching editor `code -w`"), even though every other tool on the
system accepts that value.

ADR 0015 specifies `$VISUAL`/`$EDITOR` resolution but does not address
argument handling.

## Suggested resolution

Two workable options:

1. Run through the shell like git does:
   `sh -c "$EDITOR \"$1\"" -- <path>` (value used verbatim; the note path is
   passed as a positional to avoid injecting it into the string).
2. Split the value on whitespace and treat the first token as the program,
   the rest as leading args. Simpler, no shell involved, but breaks paths
   with spaces (`EDITOR="/Applications/My Editor.app/..."`) that option 1
   handles when quoted.

Option 1 matches user expectations from git/crontab and handles quoting;
prefer it unless a no-shell policy is wanted.

## Acceptance

- `EDITOR="printf %s"`-style multi-token values launch successfully (an
  integration test can use a tiny script or `sh -c`-friendly command and
  assert the file argument arrives).
- Single-token values keep working unchanged.
