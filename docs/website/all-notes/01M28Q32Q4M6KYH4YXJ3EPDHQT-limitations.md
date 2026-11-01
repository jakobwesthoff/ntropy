---
title: Limitations
tags: [docs/reference]
site:
  order: 2
---
Two things ntropy does not do, each the result of a design choice rather
than an oversight. The decision records behind them are linked from each
section.

## macOS and Linux only

[Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md)
are real symlink trees, which is what makes them directories you can `cd`
into and browse with any tool. Windows needs Developer Mode or admin rights
to create symlinks, so ntropy targets Unix and leaves Windows out
([ADR 0020](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0020-unix-only-v1-with-soft-performance-target.md)).
There is no Windows build, no symlink fallback, and no Windows testing.
Support is deferred, not ruled out.

## Personal scale

Your files are the database. ntropy keeps no index and no daemon. Every
`search` walks `all-notes/` and parses frontmatter on demand
([ADR 0002](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0002-stateless-filesystem-scanning-over-a-derived-index.md)).
Query cost therefore grows with the vault on every run. That is fine for
hundreds to low thousands of notes, which is what ntropy is tuned for, and
not for hundred-thousand-note archives. In return everything stays plain,
greppable, committable files. Nothing can go stale or corrupt, because
there is nothing derived to invalidate. Since the filesystem stays
canonical, a cache can be added later without turning it into a database.
