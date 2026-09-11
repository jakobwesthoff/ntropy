---
name: site-themes
description: >-
  Write or adapt a site theme for the website export and the html render
  format: the theme directory layout, the custom properties the built-in
  stylesheet exposes for colors, type, and sizes, fonts, the icon sprite, and
  the page markup a stylesheet targets.
metadata:
  tags: theme, css, stylesheet, fonts, icons, tokens, site, html
---

# Site themes

A site theme changes how the exported pages look, not what they contain: the
HTML structure is ntropy's, the stylesheet, fonts, and icons are the theme's.
The same theme styles `ntropy site` and `ntropy render --to html`. Writing
one is web development: a stylesheet and the files it references, laid out
as follows.

## Layout and selection

```
<vault>/.ntropy/themes/site/<name>/
  style.css        the entry point, required; a directory without it is not a theme
  icons/*.svg      one icon per file, layered by name over the built-in set
  fonts/*          files the stylesheet references with url(fonts/...)
  anything else    copied into the site's assets/ as it is
```

```bash
ntropy site theme init mine    # copies the built-in theme to .ntropy/themes/site/mine/
                               # refuses to overwrite an existing directory
```

Select it with `[site] theme = "mine"` in `.ntropy/config.toml`, or for one
export with `--theme mine`; `--theme default` is the built-in theme
regardless of config. A configured theme that does not exist fails the
export naming the stylesheet it looked for. A file under `icons/` that is
not an SVG fails the export naming it.

## Workflow: restyle the site

1. `ntropy site theme init mine` and set `[site] theme = "mine"`.
2. Edit the custom properties at the top of `style.css`; every color,
   typeface, and size below is one of them, so a palette or a wider reading
   column is a few lines.
3. Export to a scratch directory and open it: `ntropy site -n -o /tmp/site
   --force`. Compare against the built-in look with `--theme default`.
4. Replace or add fonts under `fonts/` and declare them with `@font-face` as
   the built-in stylesheet does; the `url()`s resolve relative to the
   stylesheet.
5. Replace an icon by dropping a file with the same name into `icons/`.

Read the built-in `style.css` for anything not listed here; it is written to
be copied.

## Colors and type

Every color and typeface is a custom property on `:root`. The dark palette
redefines them under `prefers-color-scheme: dark` (guarded so an explicit
light choice wins) and under `:root[data-theme="dark"]`; the page's switch
sets `data-theme` and remembers it in the browser.

| Property | Role |
|----------|------|
| `--bg`, `--surface`, `--raised` | the page ground, a tinted surface (code, callouts, hover fills), a raised panel |
| `--border` | rules and outlines |
| `--fg-bright`, `--fg`, `--muted`, `--faint` | headings, body text, secondary text, marks |
| `--accent`, `--accent-soft` | the accent and its translucent fill (the current sidebar entry) |
| `--link`, `--note-link` | ordinary links and links to other notes |
| `--callout-note`, `--callout-tip`, `--callout-important`, `--callout-warning`, `--callout-caution` | the five callout accents |
| `--display`, `--serif`, `--sans`, `--mono` | the font stacks: page titles, note bodies, the chrome and lists, dates, counts, and code |

## Sizes

Named for their job on these pages, not as a generic scale:

| Property | Role |
|----------|------|
| `--measure` | the reading column's width |
| `--page-width`, `--gutter` | the layout's outer width and side padding |
| `--sidebar-width`, `--outline-width`, `--column-gap`, `--header-height` | the chrome's dimensions |
| `--block-gap`, `--section-gap` | between the blocks of a note, between sections |
| `--radius-control`, `--radius-block`, `--radius-panel`, `--radius-pill` | buttons and inputs; code, quotes, tables, callouts; the search panel; chips |
| `--accent-bar` | the bar on code blocks, quotes, and callouts |
| `--shadow-panel`, `--shadow-raised` | the two shadows, on the search panel and on raised elements |
| `--motion` | the length of every transition |

## Fonts

The built-in theme ships four faces under `fonts/`, all SIL Open Font
License (see `fonts/LICENSE`): Fraunces for page titles (`--display`),
Literata for note bodies and their headings (`--serif`), DM Sans for the
chrome and lists (`--sans`), DM Mono for dates, counts, and code (`--mono`).
The site copies the whole `fonts/` directory; a standalone html render
copies it beside the page.

## Icons

Every `icons/<name>.svg` becomes a `<symbol id="icon-<name>">` in a sprite
inlined into every page, and the markup shows an icon with
`<svg class="icon"><use href="#icon-<name>"/></svg>`. Your icons are layered
by name over the built-in set (Lucide, ISC, see `icons/LICENSE`): a file
with a built-in name replaces that icon, any other name adds one, a theme
without `icons/` keeps them all. The names the pages use:

- chrome: `menu`, `x`, `search`, `monitor`, `sun`, `moon`, `tag`,
  `chevron-right`, `chevron-left`;
- callouts: `info`, `lightbulb`, `message-square-warning`, `triangle-alert`,
  `octagon-alert`;
- the search palette's result kinds: `file-text`, `folder`, `tag`.

An icon takes the text color, so the stylesheet sizes and colors it through
the `.icon` class.

## Markup

The page is a `.layout` grid of:

- `.site-header` with `.site-name`, the `.search-toggle` button, and the
  `.theme-switch` of three `.theme-choice` buttons;
- `.sidebar-pane` holding `nav.sidebar`: `.nav-section` blocks, each a
  `details` with a `summary.nav-title`, an optional `.nav-all` link to the
  section's page, and `ul.nav-entries` of `li.nav-note` links and
  `li.nav-group` details whose `summary` holds the group's link (a `span`
  for a group made by hand in the nav table) and the `.chevron` icon,
  nesting with another `ul.nav-entries`; the tag section is a
  `section.nav-tags` with an `h2.nav-title` and a `ul.tag-cloud`;
- `main` with `.breadcrumbs` (a step without a page is a `span`),
  `.content`, and the `.pager`;
- `aside.side` holding `nav.outline`.

A note is a `.note-header` (`.note-title`, `.note-meta` with `.note-created`
and the `.tags`, the remaining fields as a `.frontmatter` list) followed by
`article.note-body` and, on a site page, `section.related`. Lists of notes
are `ol.note-rows` of `li.note-row` with a `time`, the `.note-row-title`
link, and `.note-row-tags`; groups are `ul.group-chips` of `.chip` links.
Callouts are `.callout.callout-<kind>` with a `.callout-title`. Highlighted
code carries Shiki's `--shiki-light` and `--shiki-dark` colors per token and
the stylesheet picks one by scheme. The current sidebar entry and outline
entry carry `aria-current`. A standalone html render has `body.document`
and no sidebar column; a site page has `body.site`.
