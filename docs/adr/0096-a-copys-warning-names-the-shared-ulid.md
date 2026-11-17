# 96. A copy's warning names the shared ULID

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0067](0067-duplicate-note-ids.md), whose "Copies are skipped"
section has the scan warn that a copy is skipped, and whose "No rename
replaces a file" section says a scan warning's text says the file was
skipped.

## Context

A full ULID used as a selector returns the keeper and its copies (ADR 0067),
so `search <ULID>` lists a copy as a row. The scan warning for that copy read
"skipped `<copy>`: its id is also used by `<keeper>`", which called a file
skipped that the same command printed.

When a full ULID selector matches the keeper and its copies, the ambiguity
error told the user to "pass a full ULID or the note's path", although a full
ULID was what had been passed.

## Decision

The scan warning for a copy reads "`<copy>` shares its id with `<keeper>`;
run `ntropy reconcile` to give it a new id". It stays a scan warning: the
commands that printed it still print it, and `--strict` still fails on it.
The scan still leaves the copy out of the notes it returns.

The warning for any other file the scan cannot use keeps its wording, "skipped
`<file>`: <reason>". The sentence of ADR 0067 about a scan warning's text
holds for those warnings, and no longer for a copy's.

When every match of an ambiguous selector carries one ULID, the error's hint
reads "these files share one ULID; pass the path of one, or run `ntropy
reconcile` to give the copies new ULIDs". Any other ambiguous selector keeps
"refine the query, or pass a full ULID or the note's path".
