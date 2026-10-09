# View leaf names are only unique within a base-name group; cross-group collisions silently drop a leaf

Found during the 2026-07-02 codebase review (unit 07, views).

## Background

Within one view group directory, each note gets a leaf symlink named
`<date>-<slug>.md`. When several notes share that base name, all of them get
a ULID-tail suffix: `<date>-<slug>-<TAIL>.md` (`src/view/leaf.rs:41-63`).
Uniqueness is only ever checked *among the notes sharing one base name*
(`disambiguating_tail_len` operates per bucket).

## Problem 1: collision between a suffixed name and a different base name

Two different base names can produce the same final filename:

- Note X: date `2026-06-25`, slug `review` — collides with a sibling, gets
  tail `123` (ULID tails are Crockford base32, which includes digits, so an
  all-digit tail is possible) → `2026-06-25-review-123.md`.
- Note Y: date `2026-06-25`, slug `review-123` (unique base, no suffix)
  → `2026-06-25-review-123.md`.

`desired_links` (`src/view/materialize.rs:106-127`) inserts both into one
`BTreeMap<PathBuf, PathBuf>` keyed by leaf path: the second insert silently
*overwrites* the first, so one of the two notes simply has no leaf in the
view. No error, no warning; the projection is silently incomplete.

## Problem 2: case-insensitive filesystems (macOS default APFS)

Slugs are lowercase; ULID tails are uppercase. On a case-insensitive
filesystem, `...review-FAV.md` (suffixed) and `...review-fav.md` (slug
ending in `-fav`) are distinct map keys but the *same* directory entry:

- `fsutil::symlink` for the second name fails with `EEXIST`, aborting the
  sync with an error, or
- the diff logic (`actual` keyed by the on-disk name, `desired` by the other
  case) sees a permanent mismatch and removes/recreates on every sync.

The same case-collapse also affects two notes whose *titles* differ only in
characters that slugify differently by case — not possible today since
slugify lowercases everything, so the tail-vs-slug clash is the only pair.

## Problem 3: duplicate note IDs never disambiguate

`disambiguating_tail_len` falls back to full ULID length when tails never
differ (`src/view/leaf.rs:70-78`). Two notes with the *same* ULID (a
hand-copied file) and same date+slug produce identical names at every
length, including the fallback — the map overwrite from Problem 1 then hides
one of them. See the companion todo on duplicate-ID detection
(`01kwh6c1ves9yjbescn8n2cms1-scan-does-not-detect-duplicate-note-ids.md`).

## Suggested resolution

After computing all leaf names for a view (across all groups is not needed —
collisions can only occur inside one group directory), enforce uniqueness of
the final `(group dir, name)` set, case-insensitively:

- detect any duplicate final path (exact or case-folded) and resolve it by
  growing the tail or appending the full ULID, or
- at minimum, surface a warning instead of silently overwriting the map
  entry.

## Acceptance

- A test where a suffixed leaf name equals another note's unsuffixed base
  name (digit-only tail) shows both notes present in the view.
- A test (or documented decision) covering the case-insensitive-filesystem
  clash between `-TAIL` and a slug ending in the same letters.
