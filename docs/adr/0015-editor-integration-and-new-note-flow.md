# 15. Editor integration and new-note flow

Date: 2026-06-24

## Status

Accepted

## Context

ntropy opens notes in the user's editor and creates new notes. It needs an
editor-resolution rule, a creation flow, and defined scope for editing more
than one note at once.

## Decision

Editor resolution: prefer `$VISUAL`, then `$EDITOR`. If neither is set, fail
with a clear message rather than defaulting to a built-in editor.

New note (`new "Title"`): create the file from the template, then open it in
the editor. `--no-edit`/`--print` instead creates the note and prints its path
only, for scripting.

When ntropy launches the editor, it reconciles the touched note on exit (slug
realignment per ADR 0004, view link updates per ADR 0008).

v1 edits a single note at a time; multi-note editing is out of scope. The
interactive picker selects one note for v1.

## Consequences

- The common path (new/open then edit) works with no flags; scripting uses
  `--no-edit`.
- No surprise editor: an unset `$VISUAL`/`$EDITOR` is an explicit error.
- Single-select picker for v1 keeps the open/reconcile flow simple; multi-edit
  can be added later.
