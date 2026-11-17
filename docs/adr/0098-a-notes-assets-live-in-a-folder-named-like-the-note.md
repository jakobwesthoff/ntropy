# 98. A note's assets live in a folder named like the note

Date: 2026-11-17

## Status

Accepted

## Context

ntropy has no concept of a note's assets. The scan ignores everything in
`all-notes/` except top-level note files (ADR 0019), so images, transcripts and
other attachments sit wherever the user or an agent put them. A vault that
kept them in `all-notes/assets/<note file name>/` lost the connection when a
`reconcile` renamed notes: the folders of two renamed notes kept their old
names and their links broke.

## Decision

A note's assets live in `all-notes/_assets/<stem>/`, where `<stem>` is the
note's file name without its extension, `<ULID>-<slug>`. The folder is named
like the note so that it stays readable and navigable without ntropy. A link
into it is a plain relative Markdown link, `_assets/<stem>/<file>`.

A folder belongs to the note whose ULID is the first 26 characters of its
name, when they are the whole name or are followed by `-`. Ownership goes by
the ULID and not by the slug, as note links resolve (ADR 0028).

`reconcile` renames each folder to the current stem of its note, rewrites the
links into the folder in every note body in the same pass, and prints the old
and the new folder name. Between an edit and the next `reconcile` the folder
keeps its old name and the links keep working, because the edit moved only the
note. `realign`, which runs after `edit` and `write`, does not touch asset
folders.

Both steps go by the ULID and not by a remembered old name. A link into a
folder name that a note owns under another name is rewritten to the name the
folder has now, when the folder with the note's stem exists and no entry of any
type in `_assets/` has the old name: a file or a symlink of that name is no stale
folder, since the link may name it. Only a reference with a `/` after the name is
rewritten, a path below the folder or the folder with a trailing slash, and a
bare `_assets/<name>` is left as it is. A `reconcile` that stopped between the rename and the rewrite is
finished by the next one, and so is a folder that drifted before ntropy
managed it. Names are compared by the directory listing, so a folder renamed
only in letter case is found on a case-insensitive filesystem as well. The
rewrite replaces `_assets/<old>/` by `_assets/<new>/` outside code regions
(ADR 0097) and keeps the spelling of the `_assets` component.

A note file is written at most once per `reconcile`, after both the asset
rewrite and the rewrite of note links, so neither undoes the other.

A rename that is not made leaves each folder as it is and counts as a warning
that names its reason: the note owns several folders, a folder of the new name
exists, or something that is not a folder has the new name. A folder whose name no note's ULID leads is not touched.

`reconcile` does nothing with asset folders in an encrypted vault. A symlinked
`_assets` or a symlinked entry in it holds no folder ntropy manages.

The summary line of `reconcile` names the asset folders moved when there are
any.
