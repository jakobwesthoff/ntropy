# 90. `--id` prints a note's ULID

Date: 2026-11-16

## Status

Accepted

Amends [ADR 0035](0035-generic-print-flag-replaces-no-edit.md), whose
`--print` was the one flag on `new`, `today` and `search` that suppressed the
editor, and the library paragraph of
[ADR 0042](0042-empty-note-creation-for-machine-authors.md), which had
`create_empty_note` return the path alone.

## Context

`new -p`, `today -p` and `search -p` print a note's path. The ULID is in the
file name, and no command printed it on its own, so a script that wanted to
refer to a note by its ULID had to cut it out of the path.

## Decision

`--id` is a flag on `search` (aliases `list` and `edit`), `new` and `today`.
It behaves like `-p` of the same command and prints the note's bare ULID
instead of its path. It has no short form.

- `search --id`: on a terminal a lone match is taken directly and several
  open the picker; a cancelled picker exits non-zero. Without a terminal
  every match prints, one ULID per line, newest first. No match exits
  non-zero as before.
- `new --id` and `today --id` create the note, or for `today` find it, and
  print its ULID without opening the editor. `today --id` and `new --id`
  refresh the views like `-p`.
- A note and its copies share one ULID (ADR 0067). Under `--id` they print
  one ULID once.
- `--id` conflicts with `-p`, and on `search` with `-P`.
- `render` and `site` have no `--id`: their `-p` prints the path of the file
  they write.
- Ctrl-Y in the picker still prints the note's path (ADR 0060).
- An id needs no escaping, so `--id` prints for a note whose file name or
  vault path `-p` refuses to print (ADR 0071).

The interface is a flag, not an `ntropy id` command. A flag reuses the
selection and the picker of the command it sits on, and `new --id` creates a
note and hands a script its ULID in the same call.

In the library, `create_empty_note` returns the new note's id together with
its path, and still no `Note`, since there is nothing to parse until content
arrives.
