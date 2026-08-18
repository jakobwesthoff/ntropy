# 43. Write command for note content

Date: 2026-08-18

## Status

Accepted

Completes the encrypted-vault command surface of
[ADR 0041](0041-opt-in-vault-encryption-with-age.md) and pairs with the empty
creation mode of
[ADR 0042](0042-empty-note-creation-for-machine-authors.md).

Departs from
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md) for this one
command: it behaves the same whether or not a controlling terminal exists.

## Context

Nothing writes a note's content except an editor. In a plaintext vault a caller
can sidestep that by editing the file, which is what `new --print` plus a file
write already does. In an encrypted vault it cannot: the file is an age
container, and text written into it is not a note ntropy can read.

The editor is not a way out for a script. Interactivity keys off the controlling
terminal (ADR 0036), so a process without one never reaches the editor at all;
`new` stamps its template and prints the path instead. `reconcile` adopts a
plaintext note dropped into an encrypted vault, but only one named
`<ulid>-<slug>.md`, which a caller may not create because the identity is not
its to assign (ADR 0004).

So authoring in an encrypted vault requires a human at a terminal, while the
same task in a plaintext vault does not.

## Decision

`ntropy write <ULID|FILENAME|PATH>` replaces one note's content with text read
from stdin. It behaves identically in both storage forms, so authoring a note is
one procedure rather than one per vault kind.

The target is named, never searched for: a full ULID, the note's filename, or a
path into the vault's `all-notes/`. It must already exist, and a ULID matching
more than one file is an error rather than a choice made on the caller's behalf.
Resolution reads the directory listing only. No note is read, which is what lets
a locked vault accept a write, since encrypting needs the public recipient alone
(ADR 0041).

The text is parsed before anything is written and refused if it is not a
well-formed note, the same validate-before-write order `new` uses for a rendered
template (ADR 0034). Identity comes from the filename and cannot be changed by
what the caller writes.

No picker and no prompt, on a terminal or otherwise. stdin already carries the
payload, so the invocation is scripted by construction, and an ambiguous target
fails instead of opening a picker in the middle of a pipeline.

After writing, the note is realigned and views refreshed, as the editor round
trip does on exit (ADR 0015). The resulting path is printed, which is how a
caller learns the new name when a written title renamed the file.

## Consequences

- An encrypted vault can be authored without an editor, including while locked.
- A caller that writes a note it has already composed reads nothing: `new
  --empty` prints a path, `write` fills it.
- A written title cannot leave the filename or the views stale, because
  reconciling is part of the command rather than a step the caller remembers.
- Each write refreshes views, which scans the vault. A plaintext vault pays that
  per write; an encrypted vault has no views and skips it.
- `write` is the first selector-taking command with no picker, so `render` and
  `delete` keep a resolution behavior it does not share.
