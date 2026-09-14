---
name: site-themes
description: >-
  Write or adapt a site theme for the website export and the html render
  format: the theme directory layout, the custom properties the built-in
  stylesheet exposes for colors, type, and sizes, fonts, the icon sprite,
  the page markup a stylesheet targets, and the page templates with their
  blocks and variables.
metadata:
  tags: theme, css, stylesheet, fonts, icons, tokens, templates, site, html
---

# Site themes

A site theme is the stylesheet, fonts, icons, and page templates of the
exported pages. The same theme renders `ntropy site` and `ntropy render --to
html`. Writing one is web development: a stylesheet and the files it
references, and templates only where the built-in layout is not enough,
laid out as follows.

## Layout and selection

```
<vault>/.ntropy/themes/site/<name>/
  style.css        the entry point, required; a directory without it is not a theme
  icons/*.svg      one icon per file, layered by name over the built-in set
  fonts/*          files the stylesheet references with url(fonts/...)
  templates/*.html page templates, replacing the built-in ones by name
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
6. Change the layout only through `templates/`: extend `ntropy/page.html`
   and fill a block, or replace a whole template (see Templates below).

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
  `details` with a `summary.nav-title` holding the section's link, and
  `ul.nav-entries` of `li.nav-note` links and
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

## Templates

The pages are rendered from three minijinja templates, `base.html` (the
document shell), `page.html` (the chrome around the content), and
`note.html` (a note's header and body). `site theme init` writes them under
`templates/`; a file there replaces the built-in template of the same name,
and a file in a subdirectory is named by its path (`partials/footer.html`).
The built-in templates stay reachable as `ntropy/<name>`, so a theme that
keeps the layout extends `ntropy/page.html` and fills its blocks, which are
all empty in the built-in page:

| Block | Where it renders |
|-------|------------------|
| `head` | after the stylesheet and the scripts, inside `<head>`; `{{ super() }}` keeps those |
| `header_nav` | in the header, after the site name |
| `header_tools` | in the header, between the search button and the scheme switch |
| `before_content`, `after_content` | inside `main`, around the breadcrumbs, the content, and the pager |
| `footer` | after the layout |
| `scripts` | at the end of the body |

```html
{# .ntropy/themes/site/mine/templates/page.html #}
{% extends "ntropy/page.html" %}
{% block header_nav %}<nav class="site-links">
{% for link in vars.links %}<a href="{{ prefix }}{{ link.href }}">{{ link.label }}</a>
{% endfor %}</nav>{% endblock %}
{% block footer %}<footer>{{ vars.copyright }}</footer>{% endblock %}
```

Every page template receives the same variables:

| Variable | What it holds |
|----------|---------------|
| `kind` | `front` (the front page), `note` (a note's own page), `group` (a tag, view, or group page, a landing note's among them), or `document` (a `render --to html` page) |
| `note` | the note the page is rendered from: `id`, `title`, `created`, `tags`, and `frontmatter` (the raw mapping, `note.frontmatter.tagline` for a field of your own); undefined on a page made of a listing alone |
| `vars` | the `[site.vars]` table of `.ntropy/config.toml`, free-form; ntropy reads nothing from it |
| `nav` | the sidebar as data: sections with `label`, `href`, `cloud`, `open`, and `items`; items with `kind` (`note` or `group`), `label`, `href` (none for a group made by hand), `current`, `open`, `count`, and their own `items` |
| `sidebar`, `outline`, `body`, `icons` | rendered HTML fragments: the sidebar, the outline, the content, the icon sprite |
| `title`, `site_title`, `lang`, `path`, `prefix` | the page's title and the site's, the `lang` attribute, the page's site-relative path (`notes/welcome.html`), and the `../` that reaches the site root |
| `stylesheet`, `scripts` | the stylesheet's `href` and every script's `src`, relative to the page |
| `breadcrumbs`, `prev`, `next` | the trail (`label`, `href`, the latter absent for a group made by hand) and the neighbours (`title`, `href`) |

Every value is escaped except the four fragments; write `{{ prefix }}` before
a site-relative link so the page works at any depth. `note.html` receives
`title`, `created`, `tags` (`label`, `href`), `fields` (`key`, `html`), and
`body`. A note picks another template with `site.template` in its
frontmatter (`template: splash` renders it with `templates/splash.html`),
which is how a landing page gets a hero or an impressum drops the sidebar; a
name the theme has no template for is an export warning, `--strict` fails on
it, and the page uses `page.html`. The templates are read at export and never
copied into the site or beside an artifact. One that fails to parse or to
render fails the export naming it, so export to a scratch directory after
every edit. A template named under `templates/ntropy/` is refused.
