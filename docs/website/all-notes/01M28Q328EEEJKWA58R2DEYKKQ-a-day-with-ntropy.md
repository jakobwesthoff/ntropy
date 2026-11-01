---
title: A day with ntropy
tags: [docs/start]
site:
  order: 1
---
A short tour through a day with the vault you just created: write a note,
find it again, read it, start a daily note, browse your notes as folders,
catch up after editing a file by hand, and turn the vault into a PDF and a
website. Run every command inside the vault.

## Write a note

Start with a thought:

```bash
ntropy new Refactor the parser
```

Your editor opens on a fresh note made from the vault's default
template, with a `title` and an empty `tags` list. Fill in the tags, add
a `status` field, and save:

```markdown
---
title: Refactor the parser
tags: [work, programming/rust]
status: in progress
---
```

Any field you add is kept, and you can search by every field. Nothing
declares `status` anywhere. Templates of your own are described in
[Templates and daily notes](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md).

## Find it again

Later, find the note by tag, by status, or by a word you half remember:

```bash
ntropy search 'tag:work and status:"in progress"'
```

The whole expression sits in single quotes, so the shell hands the inner
double quotes through to ntropy. A value with a space in it, like
`in progress`, has to arrive double-quoted. Without the outer quotes the
shell strips the inner ones. The query then asks for the status `in` and
the word `progress`, and finds nothing.

A single match opens in your editor. Several open a picker: type to
narrow the list, and Enter opens the note you meant.
[Finding notes](01M28Q32ESBVS3H2KJMRS99K87-finding-notes.md) covers the
picker, and [Query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md)
every kind of query.

## Read it without the editor

```bash
ntropy show tag:work
```

`show` prints the note in the terminal: the frontmatter on top, then the
text with its headings, lists and code styled. A note longer than the
screen opens in your pager.

## Start the day

```bash
ntropy today
```

This opens today's note. The first run of a day creates it from the
`today` template, titled with the date; every later run that day opens
the same note.

## Turn a field into a folder

Say you browse by status often. Make it a view:

```bash
ntropy view add by-status --field status
```

Now `by-status/in-progress/` is a real directory of symlinks. You can
`cd` into it, `grep` it, or open it in any editor, with no ntropy
involved. The value `in progress` became the directory `in-progress`,
because ntropy lowercases grouping values and turns spaces into dashes.
The symlink inside is named `<date>-<slug>.md` and points back at the
note's file in `all-notes/`.
[Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md)
has the rest.

## Catch up after editing by hand

ntropy refreshes the views after every change it makes itself. Open the
note's file in `all-notes/` with your editor directly, change its status
to `done`, and save. The view does not know yet, until you run:

```bash
ntropy reconcile
```

The symlink moves to `by-status/done/`. `reconcile` also renames a file
whose title you changed and fixes the links to it.

## Print it

A PDF needs [typst](https://typst.app) on your `PATH`. The Homebrew
formula installs it with ntropy; otherwise install it yourself, for
example with `brew install typst`. Then render the note and open it:

```bash
ntropy render -o /tmp/refactor-the-parser.pdf tag:work && open /tmp/refactor-the-parser.pdf
```

On Linux, use `xdg-open` in place of `open`. The PDF shows the title,
the other frontmatter fields below it with the tags as chips, and the
note's text.
[Rendering to PDF and HTML](01M28Q32H5RYMZ5MHBQSQQN13B-rendering-to-pdf-and-html.md)
covers the other formats and what decides a document's look.

## Publish the vault

```bash
ntropy site -o /tmp/notes-site && open /tmp/notes-site/index.html
```

`site` writes the whole vault as a static website that a browser opens
straight from disk: a page per note, a page per tag and per view, a
sidebar, and a search. How to shape it is in
[Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md).

