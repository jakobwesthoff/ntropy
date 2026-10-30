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

## Consequences

- Paths are resolved lexically. A symlink inside the vault is followed: a
  referenced path that is or passes through one is read and copied from
  where it points.
