# 72. Export paths stay inside the vault and the output

Date: 2026-10-30

## Status

Accepted

Extends how [ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md),
[ADR 0054](0054-site-navigation-and-url-scheme.md)
and [ADR 0057](0057-html-artifact-as-a-page-with-a-files-directory.md)
resolve and copy the files a note references.

## Context

`render --to html` and `site` copy the files a note's images and links
reference to `files/<vault path>` in their output. A relative path is
resolved against the note's directory, and one that climbs above the
vault is a warning and is not copied. A path that started with `/` was
taken as written, so a `..` in it named a file outside the vault, and
the copy's target, `files` joined with that path, left `files/`; with
more `..` parts it left the files directory and the output altogether.
A link to `/..` copied the whole directory that holds the vault.

## Decision

A local path in a note that starts with `/` is a path from the vault
root. Its `.` and `..` parts are resolved as a relative path's are, and
one that climbs above the root lies outside the vault: the export warns,
copies nothing, and the page keeps the path as written. The `typst`
emitter writes such a path as written, for the compiler to reject, as it
does a relative one (ADR 0045).

Every file the `html` engine writes into its files directory, and every
file the site export writes, must lie inside that directory. A target
that is absolute or has a `..` part fails the render or the export. The
site export checks every target before it writes the first. The check is
lexical, since every target is built from a path string.

## Consequences

- Paths and targets are checked lexically. A symlink inside the vault is
  followed: a referenced path that is or passes through one is read and
  copied from where it points.
- A target outside the output is an error, not a warning. A `render --to
  html` that fails on one may leave part of the files directory written.
  A site export checks every target first, so one that fails on such a
  target writes no file into its output directory; a forced output
  directory is emptied before the build.
