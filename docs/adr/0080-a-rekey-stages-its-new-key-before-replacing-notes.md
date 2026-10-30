# 80. A rekey stages its new key before replacing notes

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0041](0041-opt-in-vault-encryption-with-age.md), which makes
migrations crash-safe by being additive. A rekey stops being additive once it
replaces its first note.

## Context

A rekey writes every note under a freshly generated key. It generated that key
in memory and wrote it to `.ntropy/` only after the last note was replaced.
The commit phase removes each source note and renames the re-encrypted copy
into its place. From the first removal on, a crash or a failing key write left
notes that only the lost in-memory key could open, and `--resume` generated yet
another key. The two identity files were also separate writes, so even the
final step was not atomic as a pair.

## Decision

The new keypair is staged as `.ntropy/identity.pub.staged` and
`.ntropy/identity.age.staged`, and flushed, before the migration marker is
written. The marker records `staged = true`. `--resume` loads the staged
identity instead of generating one, and stops when its recipient is not the
one the marker names.

`identity.pub` and `identity.age` stay as they are until the last note is
replaced, so the old key opens the notes not yet replaced and the staged key
opens the others. Then every note must open with the new key, the marker is
rewritten with `handover = true`, `identity.age` is replaced by the staged
wrapped identity, `identity.pub` by the staged recipient, the staged files are
removed, and the marker is removed. The wrapped identity goes first because it
is the file that opens the notes. A resume that finds `handover = true`
finishes these steps without asking for any key.

A source removed before its replacement took its name leaves the note as
`<ulid>.age.rekey`, which no scan finds. A resume reads such a file with the
new key and renames it into place.

Two guards belong to the same decision. A resume refuses a marker of another
operation, since finishing a rekey as a `decrypt` removes the key files its
notes still need. `vault passphrase` refuses while a conversion is
unfinished, since installing the staged identity would undo the change.

## Rejected alternatives

- **Writing the new keypair over the old one first:** the old key then stops
  opening the notes that are not yet replaced.
