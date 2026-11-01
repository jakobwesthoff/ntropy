---
name: site-themes
description: >-
  Write or adapt a site theme for the website export and the html render
  format: the theme directory layout, the custom properties the built-in
  stylesheet exposes for colors, type, and sizes, fonts, the icon sprite,
  the page markup a stylesheet targets, what the built-in stylesheet and
  the page scripts rely on, and the page templates with their blocks and
  variables.
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
  anything else    copied as it is into the site's assets/ and beside every
                   render --to html page (icons/ into the site only)
```

Hidden files and the names an export never copies (`.git`, `.DS_Store`
and the like) stay out, and so does a file symlinked from outside the
theme and the vault. A `style.css`, icon or template symlinked from
outside them fails the theme's load with an error naming the file.

What the theme holds is what its pages get: its `style.css` is the whole
stylesheet, with nothing of the built-in one added, and its `fonts/` are
the only fonts. Icons and templates are layered by name over the built-in
ones, and a template deleted from `templates/` falls back to the built-in
one. So a theme starts from `site theme init`. That copy stays as it is
when ntropy is updated; to take a newer built-in stylesheet, init a fresh
theme under another name and carry the rules over.

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
   --force`. Compare against the built-in look with `--theme default`. A
   vault made to show a theme every kind of page:
   `curl -fsSL https://ntropy.westhoffswelt.de/downloads/theme-test-vault.tar.gz | tar xz`
   unpacks it as `theme-test/`.
4. Replace or add fonts under `fonts/` and declare them with `@font-face` as
   the built-in stylesheet does; the `url()`s resolve relative to the
   stylesheet.
5. Replace an icon by dropping a file with the same name into `icons/`.
6. Change the layout only through `templates/`: extend `ntropy/page.html`
   and fill a block, or replace a whole template (see Templates below).
   The copied `page.html` from `init` is replaced wholesale by the
   extending one; delete the copies you do not change.
7. A front page: a note named by `[site] index`, its frontmatter carrying
   `site: { template: front, hidden: true, related: false }` and the values
   the template reads; `templates/front.html` extends `page.html` and fills
   `before_content`. The website guide "Making a front page" builds one.

Add your own rules at the end of the copied `style.css`: a later rule with
the same selector wins, and a built-in rule with a more specific selector
(`blockquote .callout` over `.callout`) still wins over yours.

Read the built-in `style.css` for anything not listed here; it is written to
be copied.

## Colors and type

Every color and typeface is a custom property on `:root`. The dark palette
redefines them under `prefers-color-scheme: dark` (guarded so an explicit
light choice wins) and under `:root[data-theme="dark"]`, both inside
`@media screen`, so a printed page keeps the light palette; the page's
switch sets `data-theme` and remembers it in the browser.

| Property | Role |
|----------|------|
| `--bg`, `--surface`, `--raised` | the page ground, a tinted surface (code, callouts, hover fills), a raised panel |
| `--border` | rules and outlines |
| `--fg-bright`, `--fg`, `--muted`, `--faint` | headings, body text, secondary text, marks |
| `--accent`, `--accent-soft` | the accent and its translucent fill (the current sidebar entry) |
| `--link`, `--note-link` | ordinary links and links to other notes |
| `--callout-note`, `--callout-tip`, `--callout-important`, `--callout-warning`, `--callout-caution` | the five callout accents |
| `--display`, `--serif`, `--sans`, `--mono` | the font stacks: page titles, note bodies, the chrome and lists, dates, counts, and code |

Each `.callout-<kind>` sets `--callout` to its kind's accent, which the
callout's bar, tint, and title read.

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
  `.theme-switch` of three `.theme-choice` buttons, each naming its scheme
  in a `span.visually-hidden`;
- `.sidebar-pane` holding `nav.sidebar`: `.nav-section` blocks, each a
  `details` with a `summary.nav-title` holding the section's link, and
  `ul.nav-entries` of `li.nav-note` links and
  `li.nav-group` details whose `summary` holds the group's link (a `span`
  for a group made by hand in the nav table) and the `.chevron` icon,
  nesting with another `ul.nav-entries`; the tag section is a
  `section.nav-tags` with an `h2.nav-title` and a `ul.tag-cloud`;
- `main` with `.breadcrumbs` (a step without a page is a `span`),
  `.content`, and the `.pager` (a link into another sidebar section also has
  `.crosses-section`);
- `aside.side` holding `nav.outline`.

A note is a `.note-header` (`.note-title`, `.note-meta` with `.note-created`
and the `.tags`, the remaining fields as a `.frontmatter` list) followed by
`article.note-body` and, on a site page, `section.related`. Lists of notes
are `ol.note-rows` of `li.note-row` with a `time`, the `.note-row-title`
link, and `.note-row-tags`; groups are `ul.group-chips` of `.chip` links.
Every table sits in a `div.table-wrap` with a `data-columns` attribute holding
its column count. A column's alignment is an inline `text-align` style on
its cells, so a rule overriding it needs `!important`; a table of two
columns carries explicit ARIA roles on the table, its row groups, rows, and
cells. The built-in themes scroll a table wider than its column
inside the wrapper, with a shadow at the edge that has more to show, and draw
the rows of a two-column table as cards, the first cell a tinted strip over the
second, in a note column narrower than 32rem.

Callouts are `div.callout.callout-<kind>` with a `p.callout-title`: the
kind's English name (Note, Tip, Important, Warning, Caution) after its
icon. The marker matches in any letter case; a marker with text after it,
or another kind, stays a `blockquote`; a callout can sit inside a quote.
A link to another note is `a.note-link`; a task item starts with a disabled
checkbox `input`. Footnote references are `sup.footnote-ref` around a link
with the id `fnref-<n>`, and the footnotes follow the body as
`section.footnotes`, an `ol` of items with the id `fn-<n>`, each ending in
`a.footnote-backref`; only footnotes the body reaches are listed. The
outline is `nav.outline`, a `ul` of `li.depth-<n>` by heading level.

Code is `pre > code.language-<tag>`. In the browser a block whose language
has a grammar becomes Shiki's own `pre` (its classes start with shiki, it
carries `data-language`), each token a `span` with Shiki's `--shiki-light`
and `--shiki-dark` colors, from the Kanagawa Lotus and Kanagawa Dragon
themes; the stylesheet picks one by scheme. A block without a grammar stays
as emitted. The current sidebar entry and outline entry carry
`aria-current`. A standalone html render has `body.document` and no sidebar
column; a site page has `body.site`.

## What the built-in stylesheet relies on

A theme reworking the layout around the built-in rules keeps these:

- `.layout > main` is a container (`container-type: inline-size`); the
  card view of two-column tables is a container query and never applies
  without it. The card view is screen-only.
- `--table-cover` covers a scrolling table's edge shadows and has to equal
  the ground the table sits on; `blockquote`, `.callout`, and
  `blockquote .callout` set it. `--table-shadow` is the shadow's color.
- `.site-name` is a block with `overflow: hidden` and `text-overflow:
  ellipsis`, which ends a long header title with an ellipsis.
- Below 60rem the page is one column: the sidebar is a drawer opened by
  `.sidebar-pane:target`, the outline sits under the note, and a
  `body.document` page is one column too.
- The heading `#` follows the text in quotes and callouts, below 60rem, on
  screens without hover, and on `body.document` pages below 70rem.
- Print hides the `#` and lets a table grow; the dark palette and the dark
  code colors are screen-only, so a page prints in the light palette.
- `body` sets `overflow-wrap: break-word`, so a link, an inline code span or
  a word without a space breaks at the line end instead of widening the
  page. The containers of note text in a flex row or grid (`.frontmatter`,
  `.note-meta`, `.note-rows`, `.group-chips`, `.sidebar`, `.breadcrumbs`,
  `.palette-meta`) set `overflow-wrap: anywhere`, which also lets their items
  shrink below their longest word. `anywhere` on `body` breaks short words
  next to a long item, and `white-space: nowrap` on a tag or a number keeps
  it from breaking.
- `.visually-hidden` hides the scheme buttons' names and keeps them for
  screen readers.

## What the page scripts need

A template replacing `page.html` instead of extending it keeps the hooks of
the page script:

- buttons with `data-theme-choice` (system, light, dark); the script and the
  inline script in the head of the built-in page keep the choice in
  `localStorage` under `ntropy-theme` and set `data-theme` on `html`;
- the first element with `data-search` (the address of the search data) and
  `data-prefix` mounts the search palette, every `.search-toggle` opens it
  on a click, `/` and Ctrl+K or Cmd+K open it anyway; the palette's
  classes (`.palette-*`, `body.palette-open`, `.is-selected`) come from the
  script;
- the sidebar pane has the id `sidebar` (the drawer);
- `nav.outline` and the heading ids in `.note-body` (the outline tracking);
- `pre > code` with a `language-` class (highlighting).

## Templates

The pages are rendered from three minijinja templates, `base.html` (the
document shell), `page.html` (the chrome around the content), and
`note.html` (a note's header and body). `site theme init` writes them under
`templates/`; a file there replaces the built-in template of the same name,
and a file in a subdirectory is named by its path (`partials/footer.html`).
The built-in templates stay reachable as `ntropy/<name>`; `ntropy/page.html`
extends `base.html` and renders a note with `note.html` by those plain
names, so a theme's own copies of the two apply under it. A theme that
keeps the layout extends `ntropy/page.html` and fills its blocks.
`base.html` defines `title`, `head`, `body_class`, and `body`, which the
built-in page fills; its own six seams are empty:

| Block | Where it renders |
|-------|------------------|
| `title` | the `<title>`: the page's title, followed by the site's except on a `render --to html` page |
| `head` | inside `<head>`, holding the stylesheet, the inline scheme script, and the scripts; filling it without `{{ super() }}` drops all three |
| `body_class` | the class of `body`, `site` or `document`; `{{ super() }} front` adds a class and keeps it |
| `body` | the whole page inside `body` |
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
| `note` | the note the page is rendered from: `id`, `title`, `created`, `tags`, and `frontmatter` (the raw mapping, `note.frontmatter.tagline` for a field of your own); none on a page made of a listing alone, so test it with `{% if note %}` (`note is defined` is true there too) |
| `vars` | the `[site.vars]` table of `.ntropy/config.toml`, free-form; ntropy reads nothing from it; a TOML date, time, or datetime arrives as text in the form TOML writes it, such as `2026-11-01` or `2026-11-01T12:00:00+01:00` |
| `nav` | the sidebar as data: sections with `label`, `href`, `cloud`, `open`, and `items`; items with `kind` (`note` or `group`), `label`, `href` (none for a group made by hand), `current`, `open`, `count`, and their own `items` |
| `sidebar`, `outline`, `body`, `icons` | rendered HTML fragments: the sidebar, the outline, the content, the icon sprite |
| `title`, `site_title`, `lang`, `path`, `prefix` | the page's title; the site's title, the note's on a `document` page; the `lang` attribute; the page's site-relative path (`notes/welcome.html`); and the `../` that reaches the site root, empty on a `document` page |
| `stylesheet`, `scripts` | the stylesheet's `href` and every script's `src`, relative to the page |
| `breadcrumbs`, `prev`, `next` | the trail (`label`, `href`, the latter absent for a group made by hand) and the neighbours (`title`, `href`, `section`, the title of the neighbour's sidebar section, and `crosses_section`, true when that section is not the page's own) |

A neighbour in another sidebar section is a change of chapter. A template
can mark such a link or leave it out. This one marks it, naming the section
only when it differs from the neighbour's title:

```jinja
{% if next %}<a rel="next" href="{{ next.href }}">{% if next.crosses_section %}Next chapter{% if next.section != next.title %}: {{ next.section }}{% endif %}, {% endif %}{{ next.title }}</a>{% endif %}
```

This one leaves every link into another section out, and the whole
`nav` with it when no link remains:

```jinja
{% set earlier = prev and not prev.crosses_section %}
{% set later = next and not next.crosses_section %}
{% if earlier or later %}<nav class="pager">
{% if earlier %}<a rel="prev" href="{{ prev.href }}">{{ prev.title }}</a>{% endif %}
{% if later %}<a rel="next" href="{{ next.href }}">{{ next.title }}</a>{% endif %}
</nav>{% endif %}
```

A template prints a boolean as `True` or `False`.

Every value is escaped except the four fragments; write `{{ prefix }}` before
a site-relative link so the page works at any depth. A theme file is at
`{{ prefix }}assets/<file>` on a site page and in the files directory beside
a `render --to html` page; `{{ stylesheet | replace("style.css", "") }}<file>`
reaches it on both. A missing name, or an attribute of none, prints nothing;
one step further, such as `note.frontmatter.tagline` on a listing page, fails
the export with "undefined value", naming the template and line.
`{{ debug() }}` prints all a page receives. The filters and tests are
minijinja 3's built-in ones without `tojson` and `urlencode`, and without
minijinja-contrib; an unknown filter fails the export. `note.html` receives
`title`, `created`, `tags` (`label`, `href`), `fields` (`key`, `html`), and
`body`. A note picks another template with `site.template` in its
frontmatter (`template: splash` renders it with `templates/splash.html`),
which is how a landing page gets a hero or an impressum drops the sidebar; a
name the theme has no template for is an export warning, `--strict` fails on
it, and the page uses `page.html`. The templates are read at export and never
copied into the site or beside an artifact. One that fails to parse or to
render fails the export naming it, so export to a scratch directory after
every edit. A template named under `templates/ntropy/` is refused.
