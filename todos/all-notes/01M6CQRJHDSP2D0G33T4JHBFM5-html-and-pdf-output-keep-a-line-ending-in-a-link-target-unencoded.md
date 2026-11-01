---
title: "HTML and PDF output keep a line ending in a link target unencoded"
kind: bug
component: render
impact: low
origin: implementation
---
# HTML and PDF output keep a line ending in a link target unencoded

A character reference to a line ending, such as `&#10;` or `&#13;`, in
the target of a link or image reaches the HTML `href` and the PDF link
as a raw line ending. A browser strips it from the URL, so the link
points somewhere other than the target the note wrote. The ANSI drawing
shows such a target as `%0A` or `%0D` since patine 2.2.3 (ntropy
2.10.0), so the output formats now disagree.

## Problem

Reproduced with ntropy 2.9.0 on 2026-11-01, on the note

    [d](http://x/a&#10;b) and [c](http://x/c&#13;d)

- `render --to html` writes `href="http://x/a` LF `b"`. Browsers remove
  ASCII tab and newline from a URL, so the link opens `http://x/ab`.
  CommonMark's reference output, and comrak's `escape_href`, give
  `http://x/a%0Ab`. The CR in the second link comes out as LF too.
- `render --to pdf` writes `/URI(http://x/a\nb)` and
  `/URI(http://x/c\rd)`, a URI with a raw line ending in it.

`link_href` in `src/render/html/emitter.rs` returns a non-local
destination as written, and the HTML writer only escapes the attribute
characters. The Typst writer's `string_literal`
(`src/render/typst/writer.rs`) escapes only `\` and `"`. Other
characters pass unencoded as well (a space from `[a](<http://x/a b>)`, `ä`),
which browsers encode themselves, so only the line endings change where
a link goes.

## Suggested fix

Percent-encode CR as `%0D` and LF as `%0A` in a link or image target
before it reaches the HTML attribute and the Typst `link`/`image` call,
matching the ANSI output. Whether to go further and encode the way
comrak's `escape_href` does (spaces, non-ASCII) is part of the work:
it changes the HTML of every link that holds such characters while
browsers already treat both forms alike. Test first: one HTML and one
Typst test per line ending, red today.
