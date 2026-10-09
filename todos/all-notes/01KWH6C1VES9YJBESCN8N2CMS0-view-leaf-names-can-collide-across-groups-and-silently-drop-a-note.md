---
title: "View leaf names can collide across groups and silently drop a note"
kind: bug
component: view
origin: review
---
# View leaf names can collide across groups and silently drop a note

Found in the 2026-07-02 codebase review of views. Within one view group directory, each note gets a leaf symlink named `<date>-<slug>.md`. When several notes share that base name, each gets a ULID tail: `<date>-<slug>-<TAIL>.md` (`src/view/leaf.rs`, around lines 41 to 63). Uniqueness is only checked among notes that share a base name, so a suffixed name can equal another note's plain base name. The map that builds the view then keeps only one of the two notes.

## Problem

### Suffixed name equals another base name

Established by reading the code during the review. Note X has date `2026-06-25` and slug `review`. It collides with a sibling and gets the tail `123`, giving `2026-06-25-review-123.md`. Note Y has date `2026-06-25` and slug `review-123`, a unique base name with no suffix, giving the same filename. ULID tails are Crockford base32, which includes digits, so an all-digit tail is possible.

`desired_links` (`src/view/materialize.rs`, around lines 106 to 127) inserts both into one map keyed by leaf path. The second insert overwrites the first, so one note has no leaf in the view. There is no error and no warning.

### Case-insensitive filesystems

Slugs are lowercase and ULID tails are uppercase. On a case-insensitive filesystem such as the default macOS APFS, `...review-FAV.md` (suffixed) and `...review-fav.md` (a slug ending in `-fav`) are distinct map keys but the same directory entry. Then either `fsutil::symlink` fails with `EEXIST` for the second name and aborts the sync, or the diff logic sees a permanent mismatch, because `actual` is keyed by the on-disk name and `desired` by the other case. It removes and recreates the link on every sync. Slugify lowercases everything, so this tail-versus-slug clash is the only case-only pair that can occur today.

### Duplicate note IDs

Two notes with the same ULID (a hand-copied file) and the same date and slug produce identical names at every tail length, including the full-length fallback that `disambiguating_tail_len` uses (`src/view/leaf.rs`, around lines 70 to 78). The overwrite above then hides one of them. Detecting duplicate IDs is covered by its own todo.

## Impact

A note is missing from its view, without any error or warning.

## Suggested fix

After computing all leaf names, enforce uniqueness of the final `(group directory, name)` set, compared case-insensitively. Collisions can occur only inside one group directory, so the check does not need to span groups. For a duplicate, either grow the tail or append the full ULID. At minimum, surface a warning instead of silently overwriting the map entry.

## Open questions

The case-insensitive clash can be settled either by a test or by a documented decision. That choice is made while implementing.

## Done when

- A test where a suffixed leaf name equals another note's plain base name (digit-only tail) shows both notes in the view.
- A test, or a documented decision, covers the case-insensitive clash between `-TAIL` and a slug ending in the same letters.
