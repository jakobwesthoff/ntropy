# 24. v1 dependency selection

Date: 2026-06-24

## Status

Accepted

## Context

With features decided, the v1 crate set and the build-vs-adopt calls for each
component need recording.

## Decision

Adopt:

- `clap` (derive) — CLI parsing.
- `ulid` — ULID generation/parsing.
- `serde` + `serde_yaml_ng` — frontmatter YAML (maintained fork; upstream
  `serde_yaml` is archived). v1 only reads frontmatter (writes on `new`), so
  round-trip fidelity is not required.
- `toml` (+ `serde`) — config.
- `directories` — OS-native config locations.
- `jiff` — date/time, for rendering the ULID's UTC instant to a local date.
- `grep-searcher`, `grep-regex` — embedded full-text search.
- `ignore` — ripgrep's traversal crate, used as the directory walker with
  standard filters disabled (`standard_filters(false)`, `max_depth=1`) and its
  parallel walker, so no separate parallelism crate (`rayon`) is needed.
- `nucleo` — fuzzy matcher for the interactive picker.
- `crossterm` — terminal control for the in-house picker UI
  ([ADR 0027](0027-in-house-fuzzy-picker-over-nucleo-and-crossterm.md)).
- `unicode-width` — display-column widths for aligning the picker's columns and
  truncating rows without wide characters straddling the terminal edge.
- `thiserror` (library), `anyhow` (binary) — errors.
- Dev: `insta`, `insta-cmd`, `tempfile`/`assert_fs` — tests.

Build (no dependency):

- Query DSL tokenizer + parser.
- Template placeholder substitution.
- Frontmatter delimiter split.
- Slug/tag normalization, including German-aware transliteration.
- TTY detection via `std::io::IsTerminal`.

Plain (non-TTY) output uses no coloring; terminal styling is confined to the
interactive picker via `crossterm`.

## Consequences

- The full-text, traversal, and date crates come from one author/ecosystem
  (ripgrep/jiff), a coherent bet.
- Using `ignore`'s parallel walker removes hand-rolled parallelism.
- `serde_yaml_ng` carries fork-maintenance risk; acceptable as v1 only reads.
- Hand-rolled DSL/templates/normalization are owned and must be tested
  (ADR 0021).
