---
title: "A ULID shared by copies gets a hint asking for the full ULID it already is"
kind: bug
component: cli
impact: low
origin: review
tags: [ux]
---
# A ULID shared by copies gets a hint asking for the full ULID it already is

When a note has a copy carrying the same ULID, `delete -n -f <ULID>`
refuses with "ambiguous (2 matches)" and the hint "refine the query, or
pass a full ULID or the note's path", although a full ULID was given. In
the same vault `search -n <ULID>` warns that the copy was "skipped" and
then lists it. Found on 2026-11-17 by a reviewer of the agent-skill
rework, reproduced by hand the same day.

## Problem

Reproduced with the debug build of `9d53c7f` (`ntropy 2.11.0`) on a
scratch vault: one note, then a copy of its file under another slug with
the same ULID.

```
$ ntropy -n search <ULID>
warning: skipped `<ULID>-dup-test-copy.md`: its id is also used by `<ULID>-dup-test.md`; run `ntropy reconcile` to give it a new id
ID  DATE  TITLE  TAGS  PATH
<ULID>  2026-11-17  dup test        .../<ULID>-dup-test.md
<ULID>  2026-11-17  dup test        .../<ULID>-dup-test-copy.md
exit 0

$ ntropy -n delete -f <ULID>
warning: skipped `<ULID>-dup-test-copy.md`: ...
error: `<ULID>` is ambiguous (2 matches):
  ...
refine the query, or pass a full ULID or the note's path
exit 1
```

ADR 0067 makes a full ULID that a copy shares match the keeper and its
copies on purpose, so `delete` refusing it is intended. Two things do not
fit that rule:

- The hint names "a full ULID" as the way out, which the user just
  passed. Only the path, or `reconcile` (which the warning names), helps.
- The scan warning says the copy was skipped, yet `search` lists it as a
  second row. ADR 0067 says searches see the keeper alone, but a ULID
  selector matches the copies too; the two statements need reconciling in
  the ADR or the behaviour.

## Suggested fix

When every match shares the selector's ULID, drop "a full ULID" from the
hint and name the path or `reconcile`. Decide whether `search <ULID>` lists
copies; if it does, reword the scan warning so it does not say "skipped"
for a file the same command prints.
