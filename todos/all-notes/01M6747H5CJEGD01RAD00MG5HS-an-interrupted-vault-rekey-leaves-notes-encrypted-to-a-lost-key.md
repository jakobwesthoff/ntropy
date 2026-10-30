---
title: "An interrupted vault rekey leaves notes encrypted to a lost key"
kind: bug
component: crypto
impact: critical
origin: review
tags: [durability]
---
# An interrupted vault rekey leaves notes encrypted to a lost key

`ntropy vault rekey` replaces every note with a copy encrypted to a freshly generated key, and only afterwards writes that key to disk. If the command stops in between, through a crash, a power loss or a failing key write, every note already replaced is encrypted to a key that existed only in memory and cannot be decrypted any more. `--resume` does not help, because it generates yet another key. Found by the fsync work on 2026-10-30 and confirmed by the lead by reading the code, not reproduced.

## Problem

- `rekey` in `src/bin/ntropy/run/vault_cmd.rs` generates the new keypair (`age_io::generate_keypair`, around line 268), runs `migrate::run`, and calls `keys::write_keypair_at` only after it returns (around line 291).
- The commit phase of `migrate::run` (`src/migrate/mod.rs`, around lines 320 to 330) removes each source note (encrypted to the old key) and renames the produced note (encrypted to the new key) into its place. From the first removal on, the vault holds notes that only the in-memory key can read.
- A resumed run generates a different keypair, so it cannot read the notes the interrupted run already committed.
- `identity.pub` and `identity.age` are two separate writes, so even the final key write is not atomic as a pair.
- `docs/design/encryption.md` (around lines 151 to 156) says "the new keypair is written only once every note has been converted and verified, so an interrupted rekey can still read its own sources", which is true before the commit phase and false during it.

## Impact

Loss of the notes' content: a vault interrupted during the commit phase of a rekey has complete but undecryptable notes, and nothing in ntropy can recover them.

## Suggested fix

Store the new wrapped identity durably before the commit loop starts, for example as a staged `identity.age`/`identity.pub` pair next to the migration marker, flushed to disk, and let `--resume` load that staged key instead of generating a new one. At the end, move the staged pair into place. Make sure the old identity also stays available until every source is gone, so a resume can read either kind of note. Pin it with migration tests that stop after the first commit step and resume, and correct the design doc.
