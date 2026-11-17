# 100. Views carry an `_assets` link

Date: 2026-11-17

## Status

Accepted

Builds on [ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

A note links its assets as `_assets/<stem>/<file>`, relative to `all-notes/`
(ADR 0098). A note opened through a view entry is opened from the directory
of the symlink, and a tool that resolves relative links against that
directory finds no `_assets` there.

## Decision

Every directory of a materialized view holds an `_assets` symlink to
`all-notes/_assets/`: the view's root and each group directory, nested ones
included. The link is relative, as the leaves are, so the vault stays
relocatable. A plaintext vault that syncs views creates an empty
`all-notes/_assets/` when it is missing, so the link never dangles.

A group directory is the slug of a tag value, and a slug holds no
underscore, so no group has the name `_assets`.

A sync replaces what stands where the link belongs when that is a file that is
not a symlink, a symlink with another target, or a directory with no file in it.
A real `_assets` directory that holds files is never deleted: an editor that
opened a note through the view and saved a file relative to it leaves exactly
that, and the file may be the only copy. It keeps its place, that view
directory gets no link, and a `warning:` line names the directory wherever views
sync. The warning counts like the one for a skipped view, so `--strict` fails
on it. The sync removes the emptied directories before it creates the links.
