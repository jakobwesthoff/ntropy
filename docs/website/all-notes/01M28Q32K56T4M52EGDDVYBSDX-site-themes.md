---
title: Site themes
tags: [docs/publish]
site:
  order: 4
---
A site theme is the stylesheet, fonts, icons, and page templates of the
exported pages. The same theme styles `ntropy site` and
`ntropy render --to html`. This page is the contract a theme author
writes against: the directory layout, the custom properties of the
built-in stylesheet, its fonts and icons, the page markup, and the
templates with their blocks and variables.

## Layout and selection

A site theme is a directory `.ntropy/themes/site/<name>/` holding what a
web developer expects: a stylesheet, fonts, icons, and, where the
built-in layout is not enough, the page templates:

```
style.css        the entry point, required
icons/*.svg      one icon per file
fonts/*          files the stylesheet references with url(fonts/...)
templates/*.html the page templates, replacing the built-in ones by name
anything else    copied into the site's assets/ as it is
```

Start from the built-in theme, which has the same layout:

```bash
ntropy site theme init mine          # writes .ntropy/themes/site/mine/
```

```toml
# .ntropy/config.toml
[site]
theme = "mine"
title = "Team Docs"                  # defaults to the vault directory name
index = "01ARZ3NDEKTSV4RRFFQ69G5FAV" # the note that becomes the front page
lang = "en"
```

`site theme init` refuses to overwrite an existing directory.
`--theme <name>` overrides the configured theme for one export, and
`--theme default` returns to the built-in look. A configured theme that
does not exist fails the export.

## Colors and type

Every color and typeface of the built-in theme is a custom property on
`:root`. The dark palette redefines them under
`prefers-color-scheme: dark` and under `:root[data-theme="dark"]`;
`data-theme="light"` wins over the system preference. A theme that only
wants different colors redefines these and keeps the rest:

| Property | Role |
|----------|------|
| `--bg`, `--surface`, `--raised` | the page ground, a tinted surface (code, callouts, hover fills), a raised panel |
| `--border` | rules and outlines |
| `--fg-bright`, `--fg`, `--muted`, `--faint` | headings, body text, secondary text, marks |
| `--accent`, `--accent-soft` | the accent and its translucent fill |
| `--link`, `--note-link` | ordinary links, links to other notes |
| `--callout-note`, `--callout-tip`, `--callout-important`, `--callout-warning`, `--callout-caution` | the five callout accents |
| `--display`, `--serif`, `--sans`, `--mono` | the font stacks for page titles, note bodies, the chrome and lists, dates and code |

## Sizes

The sizes are named for their job on these pages, not as a generic
scale, so widening the reading column or flattening the corners is one
property:

| Property | Role |
|----------|------|
| `--measure` | the reading column's width |
| `--page-width`, `--gutter` | the layout's outer width and side padding |
| `--sidebar-width`, `--outline-width`, `--column-gap`, `--header-height` | the chrome's dimensions |
| `--block-gap`, `--section-gap` | between the blocks of a note, between sections |
| `--radius-control`, `--radius-block`, `--radius-panel`, `--radius-pill` | buttons and inputs; code, quotes, tables, callouts; the search panel; chips |
| `--accent-bar` | the bar on code blocks, quotes, and callouts |
| `--shadow-panel`, `--shadow-raised` | the search panel, raised elements |
| `--motion` | the length of every transition |

## Fonts

The built-in theme ships four faces under `fonts/`, all SIL Open Font
License (see `fonts/LICENSE`), declared with `@font-face` in
`style.css`:

| Face | Stack | Used for |
|------|-------|----------|
| Fraunces | `--display` | page titles |
| Literata | `--serif` | note bodies and their headings |
| DM Sans | `--sans` | the chrome and lists |
| DM Mono | `--mono` | dates, counts, and code |

Put your own files under `fonts/` and declare them the same way; the
stylesheet's `url()`s resolve relative to itself in the site's `assets/`.

## Icons

Every `icons/<name>.svg` becomes a `<symbol id="icon-<name>">` of a
sprite inlined into every page, and the markup shows an icon with
`<svg class="icon"><use href="#icon-<name>"/></svg>`. Your theme's icons
are layered by name over the built-in set (Lucide, ISC, see
`icons/LICENSE`): a file with a built-in name replaces that icon, any
other name adds one, and a theme without `icons/` keeps them all. A file
under `icons/` without an `<svg>` root fails the export naming it. An
icon takes the text color, so the stylesheet sizes and colors it through
the `.icon` class. The names the pages use:

- the chrome: `menu`, `x`, `search`, `monitor`, `sun`, `moon`, `tag`,
  `chevron-right`, `chevron-left`;
- callouts: `info`, `lightbulb`, `message-square-warning`,
  `triangle-alert`, `octagon-alert`;
- the search palette's result kinds: `file-text`, `folder`, `tag`.

## Markup

The pages share one structure, and every part of it has a class to
target. The ones a theme most often restyles:

| Part | Selectors |
|------|-----------|
| Header | `.site-header` with `.site-name`, `.search-toggle`, and the `.theme-switch` buttons |
| Sidebar | `nav.sidebar` of `.nav-section` blocks; entries in `.nav-entries` as `.nav-note` and `.nav-group`; the tag section `.nav-tags` with its `.tag-cloud` |
| Content column | `.breadcrumbs`, `.content`, `.pager` |
| A note | `.note-header` (`.note-title`, `.note-meta`, `.tags`, `.frontmatter`), `.note-body`, `.related` |
| Lists | `.note-rows` of `.note-row`; `.group-chips` of `.chip`, each with a `.count` |
| Body blocks | `.callout.callout-<kind>` with `.callout-title`; highlighted code as `.shiki`, colored per token through `--shiki-light` and `--shiki-dark` |
| Outline | `nav.outline` |
| State | `aria-current` on the current sidebar entry and outline entry; `body.site` on a site page, `body.document` on a `render --to html` page |

The built-in `style.css` styles all of them and is written to be copied,
so reading it beside an exported page is the fastest way to find any
selector not listed here.

## Templates

The pages are rendered from three
[minijinja](https://github.com/mitsuhiko/minijinja) templates:
`base.html`, the document shell; `page.html`, the chrome around the
content; and `note.html`, a note's header and body. `site theme init`
writes them under `templates/`, and a file there replaces the built-in
template of the same name; a file in a subdirectory is named by its path
(`partials/footer.html`). The built-in ones stay reachable as
`ntropy/<name>`, so a theme that only needs a row of links and a footer
extends the built-in page and fills two of its seams:

```html
{# .ntropy/themes/site/mine/templates/page.html #}
{% extends "ntropy/page.html" %}
{% block header_nav %}<nav class="site-links">
{% for link in vars.links %}<a href="{{ prefix }}{{ link.href }}">{{ link.label }}</a>
{% endfor %}</nav>{% endblock %}
{% block footer %}<footer>{{ vars.copyright }}</footer>{% endblock %}
```

```toml
# .ntropy/config.toml
[site.vars]                          # free-form; only your templates read it
copyright = "Acme, 2026"
links = [
  { label = "Docs", href = "tags/docs/index.html" },
  { label = "Legal", href = "notes/impressum.html" },
]
```

The built-in `page.html` keeps every block empty, so extending it
changes nothing until a block is filled:

| Block | Where it renders |
|-------|------------------|
| `head` | after the stylesheet and the scripts, inside `<head>` (`{{ super() }}` keeps those) |
| `header_nav` | in the header, after the site name |
| `header_tools` | in the header, between the search button and the scheme switch |
| `before_content`, `after_content` | inside `main`, around the breadcrumbs, the content, and the pager |
| `footer` | after the layout |
| `scripts` | at the end of the body |

Every page template receives the same variables:

| Variable | What it holds |
|----------|---------------|
| `kind` | `front`, `note`, `group`, or `document` (a `render --to html` page) |
| `note` | the note the page is rendered from: `id`, `title`, `created`, `tags`, and `frontmatter`, the raw mapping; undefined on a page made of a listing alone |
| `vars` | the `[site.vars]` table as it stands |
| `nav` | the sidebar as data: sections with `label`, `href`, `cloud`, `open`, and `items`; items with `kind` (`note` or `group`), `label`, `href`, `current`, `open`, `count`, and their own `items` |
| `sidebar`, `outline`, `body`, `icons` | rendered HTML fragments: the sidebar, the outline, the content, the icon sprite |
| `title`, `site_title`, `lang`, `path`, `prefix` | the page's title and the site's, the `lang` attribute, the page's site-relative path, and the `../` that reaches the site root |
| `stylesheet`, `scripts` | the stylesheet's `href` and every script's `src`, relative to the page |
| `breadcrumbs`, `prev`, `next` | the trail (`label`, `href`) and the neighbours (`title`, `href`) |

Every value is escaped except the four fragments. Write `{{ prefix }}`
before a site-relative link so the page works at any depth. `note.html`
receives `title`, `created`, `tags` (`label`, `href`), `fields` (`key`,
`html`), and `body`.

A note picks another template for its pages with `site.template` in its
frontmatter: `template: splash` renders it with `templates/splash.html`,
which is how a landing page gets a hero or an impressum drops the
sidebar. A name the theme has no template for is a warning and
`page.html` is used. The templates are read at export and never copied
into the site; one that fails to parse or to render fails the export
naming it. A theme template whose own name starts with `ntropy/` is
refused.
