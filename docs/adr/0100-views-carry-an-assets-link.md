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

While `all-notes/_assets/` exists, every directory of a materialized view
holds an `_assets` symlink to it: the view's root and each group directory,
nested ones included. The link is relative, as the leaves are, so the vault
stays relocatable. When `all-notes/_assets/` is gone, the links go with it.

A group directory is the slug of a tag value, and a slug holds no
underscore, so no group has the name `_assets`.

A sync removes anything that stands where the link belongs, a file or a
directory with its contents, and then creates the link. It removes the
emptied directories before it creates the links.
