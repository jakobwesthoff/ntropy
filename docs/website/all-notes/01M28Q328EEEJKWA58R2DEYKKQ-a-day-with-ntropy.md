---
title: A day with ntropy
tags: [docs/start]
site:
  order: 1
---
A short tour of how the pieces fit together: write a note, find it again, turn
a field into a browsable folder, and catch the vault up after editing behind
ntropy's back. Every command here is in the
[Commands](01M28Q3292T4MPPX6NZN69PN6H-commands.md) reference.

## Write a note

Start with a thought:

```bash
ntropy new Refactor the parser
```

Your editor opens on a fresh note stamped from the `default.md`
[template](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md). The
template already gives it a `title` and an empty `tags` list. Fill in the
tags, add a `status` field, and save:

```markdown
---
title: Refactor the parser
tags: [work, programming/rust]
status: in progress
---
```

Any field you add is kept as it is, and every field is filterable by
existing. Nothing declares `status` anywhere.

## Find it again

Later, find the note by tag, by status, or by a word you half remember:

```bash
ntropy search 'tag:work and status:"in progress"'
```

The whole expression sits in single quotes so the shell hands the inner
double quotes through to ntropy; a value containing a space, like
`in progress`, has to arrive double-quoted. Without the outer quotes the shell
strips the inner ones and the query fails with a syntax error.

A single match opens straight in your editor. Several drop you into the
[interactive picker](01M28Q32FZCNHV2PZ5FSG8E360-the-interactive-picker.md),
where you narrow the list by typing and open the one you meant with Enter. The
[query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md) has the rest of
the grammar.

## Turn a field into a folder

Say you browse by status often. Make it a
[materialized view](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md):

```bash
ntropy view add by-status --field status
```

Now `by-status/in-progress/` is a real directory of symlinks. You can `cd`
into it, `grep` it, or open it in any editor, with no ntropy involved. The
value `in progress` became the directory `in-progress` because ntropy
lowercases and hyphenates grouping values, and the symlink inside is named
`<date>-<slug>.md` and points back at the note's file in `all-notes/`.

## Catch up after edits outside ntropy

ntropy refreshes the views after every change it makes itself. Change a note's
`status` straight in your editor, without going through ntropy, and the views
do not know until you tell them:

```bash
ntropy reconcile
```

That moves the symlink to `by-status/done/` (or wherever the new value
belongs), realigns any filename whose slug drifted from its title, and
re-syncs every view. The vault is back in step.
