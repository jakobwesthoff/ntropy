# 28. Note-to-note links as standard Markdown links

Date: 2026-06-25

## Status

Accepted

## Context

ADR 0004 fixed note identity as the filename ULID and stated that links resolve
by globbing `<ulid>-*.md`, but left the in-body link syntax undefined. Notes
need a way to reference each other that survives title and slug edits and stays
readable, portable Markdown.

## Decision

A note-to-note link is a standard Markdown inline link whose target is the
current filename of the target note, relative within `all-notes/`:

    [display text](<ulid>-<slug>.md)

- The leading 26 characters of the target are the ULID and carry identity
  (ADR 0004); the `<slug>` and `.md` make the target a real, clickable path.
- **Resolution:** parse the first 26 characters of the target as a ULID and
  glob `<ulid>-*.md`. A target whose prefix is not a valid ULID, or whose ULID
  matches no note, is not an ntropy link and is left untouched. This ignores
  external links, in-document anchors and images.
- Because the target is the real on-disk filename, links open in any standard
  Markdown editor or viewer without ntropy.
- **reconcile** gains a pass that rewrites the slug portion of link targets to
  the target note's current filename, keyed on the ULID. As with filename
  realignment (ADR 0004), ntropy only does this on explicit `reconcile`, never
  on a stray edit.
- **Backlinks are computed on demand** by scanning bodies for the target ULID.
  They are never stored in frontmatter.

### Rejected alternatives

- **Wikilinks (`[[...]]`):** not standard Markdown and not click-to-open in a
  plain viewer.
- **ULID-only target (`[..](<ulid>.md)`):** stable but not a real path, so not
  clickable.
- **Backlinks stored in frontmatter:** duplicates derived data inside source
  files (against ADR 0002 and the single-copy principle of ADR 0004) and
  rewrites the target note's file for an edit that is not about its content,
  churning its modification time.

## Consequences

- Links render and navigate in any Markdown tool, not just ntropy.
- The body is no longer fully opaque: a link-extraction regex pass is introduced
  for resolution, reconcile rewriting and backlinks. This is a regex pass, not
  Markdown AST parsing; ntropy still ships no Markdown parser.
- reconcile gains a link-target rewrite step.
- Between reconcile runs a renamed target leaves a link's slug stale. The link
  still resolves by ULID, but click-to-open in a plain viewer breaks until
  reconcile refreshes the slug.
</content>
</invoke>
