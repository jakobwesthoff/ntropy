---
title: "Print only a note's ULID for scripts"
kind: feature
component: cli
status: needs-discussion
origin: request
tags: [ux]
---
# Print only a note's ULID for scripts

User feedback from 2026-11-16: a script that renders a note wants to pin
it by its ULID, so the command keeps working when the search string later
starts to match a second note. No command prints only the ULID today. The
user expected `show` or something like an `id` command to give it.

## Context

`search -p` (alias `list -p`) prints the note's full path, and the ULID
can be cut out of the filename. That works but is not something a script
should have to parse. With `-n`, `render` and `show` already fail when a
query matches more than one note (`src/bin/ntropy/run/one_note.rs`,
`select_one`), so an ambiguous query does not render the wrong note
silently. The ask is about a stable reference, not about that failure.

## Options

- An `ntropy id <ID|QUERY>` command that prints the ULID of the one
  matching note and fails on zero or several matches, like `render -n`.
- A flag on `search` that prints ULIDs instead of paths, one per line,
  next to `-p` and `-P`.

The two can coexist. Letting `render` and `show` take a note's path, as
`write` already does, would cover part of the same need.

## Open questions

- Which of the options, or both?
- Does the structured output of the JSON todo make a dedicated flag
  unnecessary, or is a plain one-ULID output still worth having for
  shell use?

## Relations

- Relates to: [Add structured JSON output for machine consumers](01KW9Z4FJQNVJSTFEM8A6Q1MAA-add-structured-json-output-for-machine-consumers.md), a script could read `id` from it
