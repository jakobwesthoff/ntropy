# 101. Renders and exports serve only a note's own assets

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0073](0073-what-html-and-site-exports-copy-from-the-vault.md),
whose list of what an export refuses gains the asset folders of other notes.
Builds on [ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md).

## Context

A note has only its own assets (ADR 0098). `reconcile` reports a link into
another note's folder (ADR 0099), and a render or an export that follows the
link would still put another note's file into the artifact. A linked directory
is mirrored with its whole tree, so a link to `all-notes/` or the vault root
reaches every asset folder.

## Decision

A file in another note's asset folder is refused by every render and export
that embeds or copies files: `typst` and `pdf`, `html` and `site`. A folder is
another note's when its name does not lead with the ULID of the note being
rendered. A folder no note owns, and a file directly in `_assets/`, is refused
too. An `_assets` directory elsewhere in the vault, such as `projects/_assets/`,
is a user's own directory and is not refused unless the folder below it is
owned by another note, which is how a view's `_assets` link (ADR 0100) is
reached.

A refused link is a warning, and `--strict` fails on it. The `typst` and `pdf`
formats show the link text, or the alt text of an image, in place of the link
or the picture and warn "`<path>` is in another note's asset folder; not
rendered". The `html` and `site` formats keep the link as written, do not copy
the file and warn "referenced path `<path>` is in another note's asset folder;
not copied". The `ansi` format embeds and copies nothing and is not affected.

The check judges the path as written, including a case variant of `_assets` and
a fragment after the path. The `html` and `site` formats also judge the path
the file resolves to, so a symlink in a note's own folder that leads into
another folder is refused. The typst engine touches no filesystem and judges the
path as written, so such a symlink is not seen there. That is the class of
symlinks that todo 01M66B2JM5XQDK7M3ZVKA1NZJV tracks for images in PDF renders,
and it stays open.

Links to files outside `all-notes/_assets/`, beside the notes or in a
vault-root `assets/` folder, render and export as before.

A linked directory that contains the vault's `all-notes/_assets/` does not
mirror it: the walk leaves that directory out and the warning for the directory
says so. A symlink the walk meets whose target lies in another folder of
`_assets/` than the directory it stands in, or in any folder when it stands
outside `_assets/`, is left out the same way: it would carry a file of another
note's folder into the walk of a note's own folder. A symlink to a file in the
same folder is copied. A note's own folder is copied by linking the folder or a file in it,
which starts the walk below `_assets/`.
