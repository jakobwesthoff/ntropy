# 4. Note identity and filename strategy

Date: 2026-06-24

## Status

Accepted

## Context

A note needs a stable identity that survives title edits so links do not
break. The title is part of the filename for readability, so a
title-independent component must carry identity. Storing identity in two
places (filename and frontmatter) introduces a copy that can drift.

## Decision

Filename: `<ulid>-<slug>.md`.

- `<ulid>`: 26-char Crockford base32 ULID, generated at creation, fixed-width,
  parseable by position and matchable by glob. It is the canonical identity.
- `<slug>`: normalized title (rules TBD).

The filename ULID is the single source of truth for identity. ntropy parses
`id` from the filename at read-time and never writes it to frontmatter, so
there is no second copy to sync. Links resolve by globbing `<ulid>-*.md`, with
no frontmatter parsing.

The readable creation date is not in the filename; it is derived from the ULID
and rendered only at display time, keeping a timezone rendering out of the
immutable name.

The canonical title lives in frontmatter (ADR 0005); the slug derives from it.
When ntropy launches the editor and the title changes, it regenerates the slug
and renames the file on exit, keeping the ULID. For out-of-band edits,
filenames realign only on explicit `reconcile`. ntropy never renames silently
on a stray edit.

## Consequences

- One copy of identity, no sync step, no drift.
- Identity is as durable as the filename and survives malformed frontmatter.
- Hand-mangling a filename's ULID prefix breaks that note's identity and its
  links; treated as user error on a tool-managed file.
- Slug normalization rules remain open.
