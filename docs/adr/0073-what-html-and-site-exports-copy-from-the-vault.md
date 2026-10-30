# 73. What html and site exports copy from the vault

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0072](0072-export-paths-stay-inside-the-vault-and-the-output.md),
which decides which referenced paths `render --to html` and `site` copy.

## Context

`render --to html` and `site` copy the files a note references to
`files/<vault path>` in their output, and a referenced directory with its
whole tree ([ADR 0057](0057-html-artifact-as-a-page-with-a-files-directory.md));
`site` copies the selected theme's directory the same way. A link to `/`
copied the vault's `.ntropy/`, with its configuration and, in an encrypted
vault, the passphrase-protected identity, and, in a vault that is a git
checkout, `.git/`, whose history holds deleted notes and notes the export
leaves out. A note that carries such a link may come from someone else.

## Decision

This decision covers the html and site exports: the files notes reference
and the files of a site theme.

### Names never copied

An export never copies an entry whose name is on one list, nor anything
below it. The list holds metadata and history that can carry notes or the
vault's configuration, or reveal other files:

- a note tool's configuration or pointer: `.ntropy`, `.ntropy-vault`,
  `.obsidian`;
- version control, whose history holds deleted and unexported notes:
  `.bzr`, `.fslckout`, `.git`, `.hg`, `.jj`, `.pijul`, `.svn`, `CVS`,
  `_darcs`;
- a sync tool's state: `.dropbox.cache`, `.stfolder`, `.stversions`;
- trash: `$RECYCLE.BIN`, and every name starting with `.Trash`, which
  covers `.Trashes`, `.Trash-1000` and Obsidian's `.trash`;
- credentials: `.aws`, `.env`, `.gnupg`, `.netrc`, `.ssh`;
- file-manager metadata that reveals other files: the Finder's `.DS_Store`
  names a folder's files, Windows Explorer's `Thumbs.db` holds thumbnails of
  its images.

Each name matches a directory or a file, at any depth below the linked
directory or the theme. Names are compared without regard to ASCII case,
since a case-insensitive filesystem such as APFS or NTFS opens `.NTROPY` as
`.ntropy` and [ADR 0068](0068-unique-view-leaf-names-per-directory-ignoring-case.md)
treats names that differ only in case as one. `CVS` is the exception and
matches its exact case, because `CVs` is a plausible folder of résumés;
macOS resolves a path to the case stored on disk, so a `cvs` written in a
note is still caught there through the path it resolves to. The name is
checked on the path as written and on the path it resolves to, so a
symlink to a listed entry is refused too. The list is `NEVER_COPIED` in
`src/site/scope.rs`, the one place it is kept; a name that fits none of the
categories needs a category there first.

### Hidden entries

When a directory is mirrored, every entry whose name starts with `.` is left
out, at any depth below the linked directory. A link that names a hidden
file copies it unless the name is listed. A link that names a hidden
directory mirrors it, without the hidden entries inside it. A theme's
hidden files are left out the same way.

### Reporting

A link to a listed path copies nothing, gets a warning, and the page keeps
the link as written, as a path outside the vault does (ADR 0072).

Inside a mirrored directory or a theme, what is left out is reported once
per linked directory, at one of two levels. A `.ntropy` entry is a warning,
naming its path. Hidden entries and the other listed entries are a `note:`
line that counts them and names up to five distinct listed names. A skip
that protects against a hostile or surprising link counts for `--strict`;
routine metadata inside a folder, such as the `.DS_Store` the Finder leaves
in nearly every folder, does not, so a `note:` line never fails a strict
run. A linked directory that ends up copying nothing is a warning.
Overlapping links, such as `/` and `/assets`, report per link; each file is
copied once.

The export's own output is never mirrored: the artifact and its files
directory for `render --to html`, the output directory for `site`.

## Consequences

- Hard links into `.ntropy` are not caught. git and sync tools do not carry
  hard links, and creating one takes local write access to the vault.
- Earlier renders and site exports stored inside the vault are ordinary
  files and are copied by a directory link that reaches them. `render` and
  `site` warn about an output inside the vault only for an encrypted vault.
