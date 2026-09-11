---
title: Limitations
tags: [docs/develop]
site:
  order: 1
---
Three things ntropy does not do, each the result of a design choice rather
than an oversight. The decision records behind them are linked from each
section.

## macOS and Linux only

[Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md)
are real symlink trees, which is what makes them directories you can `cd`
into and browse with any tool. Windows needs Developer Mode or admin rights
to create symlinks, so ntropy targets Unix and leaves Windows out
([ADR 0020](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0020-unix-only-v1-with-soft-performance-target.md)).
There is no Windows build, no symlink fallback, and no Windows testing.
Support is deferred, not ruled out. The open questions are the symlink
privilege, path and case-folding differences, and the missing `SIGPIPE`.

## Personal scale

Your files are the database. ntropy keeps no index and no daemon. Every
`list`, `search`, and `filter` walks `all-notes/` and parses frontmatter on
demand
([ADR 0002](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0002-stateless-filesystem-scanning-over-a-derived-index.md)).
Query cost therefore grows with the vault on every run. That is fine for
hundreds to low thousands of notes, which is what ntropy is tuned for, and
not for hundred-thousand-note archives. In return everything stays plain,
greppable, committable files, and nothing can go stale or corrupt, because
there is nothing derived to invalidate. Since the filesystem stays
canonical, a cache can be added later without turning it into a database.

## Views can drift on out-of-band edits

ntropy refreshes the view trees after its own mutations (create, edit,
retitle, delete). Change frontmatter or rename files behind its back, with
`$EDITOR` or a script, and the views will not catch up until the next
`ntropy reconcile`. That one command syncs the view trees, realigns
filenames whose slug drifted from the title, and rewrites links that
pointed at the old filenames, as described under
[Commands](01M28Q3292T4MPPX6NZN69PN6H-commands.md). The notes themselves
are never stale, only the derived views.
