# 87. Heading ids avoid the ids of the page

Date: 2026-11-01

## Status

Accepted

Extends [ADR 0049](0049-shared-markdown-walk-with-typst-and-html-emitters.md),
whose walk hands out the heading ids.

## Context

A heading's id followed GitHub's rule and was made unique only among the
headings of its body. On a site page it could equal an id the page uses for
something else: the navigation pane is `#sidebar`, the icon sprite holds an
`icon-<name>` symbol per icon, and the footnotes are `fn-<n>` and
`fnref-<n>`. A link to such a heading reached the other element. A heading
inside a footnote definition took an id and a duplicate suffix whether or
not the definition was drawn.

## Decision

A heading id is unique among all ids of the page, and only drawn headings
get one.

- An output names the ids it reserves. A heading whose id is reserved takes
  the next free `-1`, `-2`, and so on, the way a repeated heading does. The
  set must be finite, so the search for a free id ends.
- The HTML output reserves `fn-1` to `fn-K` and `fnref-1` to `fnref-K`, with
  `K` the number of footnote definitions in the body, and the ids its
  targets name. The site reserves `sidebar` and the icon ids. A
  `render --to html` page has no sidebar and reserves only the icon ids.
- A heading inside a footnote definition takes its id when the definition is
  drawn, after the headings of the body. A definition that is never drawn,
  or that a later definition with the same label replaces, leaves no id,
  suffix or outline entry.
- Ids an exported page's own theme templates add are the theme's to keep
  apart from heading ids.
