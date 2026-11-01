---
title: Site themes
tags: [docs/publish]
site:
  order: 7
---
A site theme is the stylesheet, fonts, icons, and page templates of the
exported pages. The same theme styles `ntropy site` and
`ntropy render --to html`. This page is the contract a theme author
writes against: the directory layout, the custom properties of the
built-in stylesheet, its fonts and icons, the page markup, what the
stylesheet and the page scripts rely on, and the templates with their
blocks and variables.

## Layout and selection

A site theme is a directory `.ntropy/themes/site/<name>/` holding what a
web developer expects: a stylesheet, fonts, icons, and, where the
built-in layout is not enough, the page templates:

```
style.css        the entry point, required
icons/*.svg      one icon per file
fonts/*          files the stylesheet references with url(fonts/...)
templates/*.html the page templates, replacing the built-in ones by name
anything else    copied as it is into the site's assets/ and beside
                 every render --to html page
```

`icons/` goes into the site's `assets/` only; both kinds of page inline
the icon sprite. Hidden files, and the names a
[website export](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md)
never copies such as a theme's `.git`, stay out, as does a file that is a
symlink to somewhere outside the theme and the vault. If `style.css`, an
icon or a template is such a symlink, the theme does not load and the
error names the file.

What a theme holds is what its pages get. Its `style.css` is the whole
stylesheet: nothing of the built-in stylesheet is added, and its `fonts/`
are the only fonts. Icons and templates are layered by name instead: an
icon or a template the theme does not have is the built-in one. So start
from the built-in theme, which has the same layout:

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

The copy `site theme init` writes stays as it is when ntropy is updated.
To take a newer built-in stylesheet, write a fresh theme under another
name with `site theme init` and carry your rules over to it. A template
you delete from `templates/` falls back to the built-in one of that name.

## Colors and type

Every color and typeface of the built-in theme is a custom property on
`:root`. The dark palette redefines them under
`prefers-color-scheme: dark` and under `:root[data-theme="dark"]`, both
inside `@media screen`, so a printed page keeps the light palette;
`data-theme="light"` wins over the system preference. A theme copied
with `site theme init` that only wants different colors changes these
and nothing else:

| Property | Role |
|----------|------|
| `--bg`, `--surface`, `--raised` | the page ground, a tinted surface (code, callouts, hover fills), a raised panel |
| `--border` | rules and outlines |
| `--fg-bright`, `--fg`, `--muted`, `--faint` | headings, body text, secondary text, marks |
| `--accent`, `--accent-soft` | the accent and its translucent fill |
| `--link`, `--note-link` | ordinary links, links to other notes |
| `--callout-note`, `--callout-tip`, `--callout-important`, `--callout-warning`, `--callout-caution` | the five callout accents |
| `--display`, `--serif`, `--sans`, `--mono` | the font stacks for page titles, note bodies, the chrome and lists, dates and code |

Each `.callout-<kind>` sets `--callout` to its kind's accent, and the
callout's bar, tint, and title read `--callout`.

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
| `--radius-control`, `--radius-block`, `--radius-panel`, `--radius-pill` | images, the scheme buttons, search results, sidebar groups and note rows; code, quotes, tables, callouts, the search button and the scheme switch; the search panel; chips |
| `--accent-bar` | the bar on code blocks, quotes, and callouts |
| `--shadow-panel`, `--shadow-raised` | the search panel and the phone's sidebar drawer; the chosen scheme button |
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
stylesheet's `url()`s resolve relative to itself, in the site's `assets/`
and in a rendered page's files directory alike.

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
| Header | `.site-header` with `.site-name`, `.search-toggle`, and the `.theme-switch` of three `button.theme-choice`, each naming its scheme in a `span.visually-hidden` |
| Sidebar | `nav.sidebar` of `.nav-section` blocks; entries in `.nav-entries` as `.nav-note` and `.nav-group`; the tag section `.nav-tags` with its `.tag-cloud` |
| Content column | `.breadcrumbs`, `.content`, `.pager` (a link into another sidebar section also has `.crosses-section`) |
| A note | `.note-header` (`.note-title`, `.note-meta`, `.tags`, `.frontmatter`), `.note-body`, `.related` |
| Lists | `.note-rows` of `.note-row`; `.group-chips` of `.chip`, each with a `.count` |
| Links | `a.note-link` for a link to another note, a plain `a` for any other |
| Callouts | `div.callout.callout-<kind>` holding a `p.callout-title` and the body. The title is the kind's English name, Note, Tip, Important, Warning, or Caution, after the icon `info`, `lightbulb`, `message-square-warning`, `triangle-alert`, or `octagon-alert`. The marker matches in any letter case; a marker with text after it on its line, or a kind outside these five, leaves an ordinary `blockquote`. A callout can sit inside a quote |
| Code | `pre > code.language-<tag>`. In the browser, a block whose language has a grammar becomes Shiki's `pre` with the classes `shiki shiki-themes` and a `data-language` attribute; every token is a `span` carrying its two colors as `--shiki-light` and `--shiki-dark`. The colors are those of the Kanagawa Lotus (light) and Kanagawa Dragon (dark) themes; a stylesheet picks one per scheme, as the built-in one does, or sets its own. A block without a grammar stays as it is |
| Task lists | a disabled `input[type="checkbox"]` at the start of the item |
| Tables | `div.table-wrap[data-columns="<n>"]` around every `table`. A column's alignment is an inline `style="text-align: …"` on its cells, so a rule that overrides it needs `!important`. A table of two columns carries explicit ARIA roles on the table, its row groups, rows, and cells. The built-in themes scroll a table wider than its column inside the wrapper, with a shadow at the edge that has more to show, and draw each row of a two-column table as a card, the first cell a tinted strip over the second, in a note column narrower than 32rem |
| Footnotes | `sup.footnote-ref` around a link with the id `fnref-<n>` at a reference; `section.footnotes` after the body, an `ol` of `li#fn-<n>`, each ending in `a.footnote-backref`. Only the footnotes the body reaches are listed, numbered by their first reference on the page |
| Heading link | `a.heading-anchor` at the end of every heading in a note body, holding a `#` in a `span`; the built-in themes show it in the margin on hover and on keyboard focus. `.heading-anchor { display: none }` hides it |
| Outline | `nav.outline`, a `ul` of `li.depth-<n>`, n being the heading level |
| State | `aria-current` on the current sidebar entry and outline entry; `body.site` on a site page, `body.document` on a `render --to html` page |

Heading ids never equal an id of the page around them: ntropy suffixes a
heading that would take `sidebar`, an icon's `icon-<name>` or a footnote's
`fn-<n>` or `fnref-<n>`. An id that a template of your own theme puts on a
note page, or that an icon's SVG defines inside its symbol, is yours to keep
apart from heading ids.

The built-in `style.css` styles all of them and is written to be copied,
so reading it beside an exported page is the fastest way to find any
selector not listed here.

## What the built-in stylesheet relies on

A theme that keeps the built-in rules and reworks the layout around them
keeps these, or the matching behavior goes:

- `.layout > main` is a container (`container-type: inline-size`). The
  card view of two-column tables is `@container (max-width: 32rem)` and
  never applies without it. The card view is inside `@media screen`.
- `--table-cover` is the color the edge shadows of a scrolling table are
  covered with, so it has to equal the ground the table sits on.
  `blockquote`, `.callout`, and `blockquote .callout` set it to their
  ground. `--table-shadow` is the shadow's color.
- `.site-name` is a block with `overflow: hidden` and
  `text-overflow: ellipsis`, which ends a title that does not fit the
  header with an ellipsis.
- Below 60rem the layout is one column: the sidebar becomes a drawer that
  `.sidebar-pane:target` opens, the outline moves under the note, and a
  `render --to html` page is one column too.
- The heading `#` follows the heading's text inside quotes and callouts,
  below 60rem, on screens without hover, and on `render --to html` pages
  below 70rem.
- `@media print` hides the `#` and lets a table grow to its full width.
  The dark palette and the dark token colors of code are inside
  `@media screen`, so a page prints in the light palette.
- `body` sets `overflow-wrap: break-word`, so a link, an inline code span or
  a word without a space breaks at the line end instead of widening the
  page. The containers of note text that sit in a flex row or grid
  (`.frontmatter`, `.note-meta`, `.note-rows`, `.group-chips`, `.sidebar`,
  `.breadcrumbs`, `.palette-meta`) set `overflow-wrap: anywhere`, which also
  lets their items shrink below their longest word. Setting `anywhere` on
  `body` breaks short words next to a long item, and `white-space: nowrap`
  on a tag or a number keeps it from breaking.
- `.visually-hidden` hides the names of the scheme buttons and keeps them
  for screen readers.

## What the page scripts need

The page script `app.js` finds its parts by these hooks. A template that
replaces `page.html` instead of extending it keeps them:

- the scheme switch: buttons with `data-theme-choice` set to `system`,
  `light`, or `dark`. The script and the inline script in the `<head>` of
  the built-in `page.html` keep the choice in `localStorage` under
  `ntropy-theme` and set `data-theme` on the `html` element;
- the search: the first element with `data-search` (the address of
  `search-data.js`) and `data-prefix` mounts the search palette, every
  `.search-toggle` opens it on a click, and `/` and Ctrl+K (Cmd+K on a
  Mac) open it anyway. The palette's own classes, `.palette-*`,
  `body.palette-open`, and `.is-selected`, come from the script;
- the drawer: the sidebar pane has the id `sidebar`;
- the outline: `.outline`, and the ids of the headings in `.note-body`;
- highlighting: `pre > code` with a `language-<tag>` class.

## Testing a theme

The theme test vault is a small vault made to show a site theme the kinds
of page and the parts of a page the built-in theme draws. Its front page,
Welcome, is set by `[site] index` and lists what each note shows. The theme
test note holds the parts of a note body, with nested quotes, callouts and
tables, a frontmatter table with nested values, and an outline with all six
heading levels; a companion note is the target of a link with a heading
anchor. A landing note, "Site tour", makes a group page with a listing,
three tour stops give the sidebar order and the pager, "Tour detail" has a
breadcrumb of three steps, "Raw HTML and remote images" holds what only web
pages show, and a hidden note has a page that no sidebar or listing names.
A note whose title, tag, frontmatter key and frontmatter value are each one
word wider than a phone screen shows how a theme wraps unbreakable text. The
view `by-status` and the tags fill the sidebar.

Download it, copy a theme into it, and export it with that theme:

```bash
curl -fsSL https://ntropy.westhoffswelt.de/downloads/theme-test-vault.tar.gz | tar xz
mkdir -p theme-test/.ntropy/themes/site
cp -R <vault>/.ntropy/themes/site/mine theme-test/.ntropy/themes/site/
ntropy --vault theme-test site -o theme-test-site --force --theme mine
```

Every [ntropy release](https://github.com/jakobwesthoff/ntropy/releases/latest)
also carries `theme-test-vault.tar.gz`, packed from the same commit as its
binary. Welcome also names what the vault cannot show: the generated front
page without `index`, a breadcrumb step without a link, which only a
`[[site.nav]]` table makes, an empty listing, and the parts the page's
scripts draw. To have the theme test note in your own vault, for a
`render --to html` page of it,
[Writing a site theme](01M6CBSVXZXEW713FS476VB76F-writing-a-site-theme.md#before-you-start)
gives the command.

## Templates

The pages are rendered from three
[minijinja](https://github.com/mitsuhiko/minijinja) templates:
`base.html`, the document shell; `page.html`, the chrome around the
content; and `note.html`, a note's header and body. `site theme init`
writes them under `templates/`, and a file there replaces the built-in
template of the same name; a file in a subdirectory is named by its path
(`partials/footer.html`). The built-in ones stay reachable as
`ntropy/<name>`. `ntropy/page.html` extends `base.html` and renders a
note with `note.html` by those plain names, so a theme's own `base.html`
and `note.html` apply under it too. A theme that only needs a row of
links and a footer extends the built-in page and fills two of its seams:

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

`base.html` defines the blocks `title`, `head`, `body_class`, and
`body`. The built-in `page.html` fills those four and adds six empty
ones at its seams, so extending it changes nothing until a block is
filled:

| Block | Where it renders |
|-------|------------------|
| `title` | the `<title>`: the page's title, followed by the site's except on a `render --to html` page |
| `head` | inside `<head>`, holding the stylesheet, the inline scheme script, and the scripts; filling it without `{{ super() }}` drops all three |
| `body_class` | the class of `body`: `site`, or `document` on a `render --to html` page |
| `body` | the whole page inside `body` |
| `header_nav` | in the header, after the site name |
| `header_tools` | in the header, between the search button and the scheme switch |
| `before_content`, `after_content` | inside `main`, around the breadcrumbs, the content, and the pager |
| `footer` | after the layout |
| `scripts` | at the end of the body |

Every page template receives the same variables:

| Variable | What it holds |
|----------|---------------|
| `kind` | `front`, `note`, `group`, or `document` (a `render --to html` page) |
| `note` | the note the page is rendered from: `id`, `title`, `created`, `tags`, and `frontmatter`, the raw mapping; none on a page made of a listing alone |
| `vars` | the `[site.vars]` table as it stands; a TOML date, time, or datetime arrives as text in the form TOML writes it, such as `2026-11-01`, `12:00:00`, `2026-11-01T12:00:00`, or `2026-11-01T12:00:00+01:00` |
| `nav` | the sidebar as data: sections with `label`, `href`, `cloud`, `open`, and `items`; items with `kind` (`note` or `group`), `label`, `href`, `current`, `open`, `count`, and their own `items` |
| `sidebar`, `outline`, `body`, `icons` | rendered HTML fragments: the sidebar, the outline, the content, the icon sprite |
| `title`, `site_title`, `lang`, `path`, `prefix` | the page's title; the site's title, the note's on a `document` page; the `lang` attribute; the page's site-relative path; and the `../` that reaches the site root |
| `stylesheet`, `scripts` | the stylesheet's `href` and every script's `src`, relative to the page |
| `breadcrumbs`, `prev`, `next` | the trail (`label`, `href`) and the neighbours (`title`, `href`, `section`, the title of the neighbour's sidebar section, and `crosses_section`, true when that section is not the page's own) |

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

Every value is escaped except the four fragments. Write `{{ prefix }}`
before a site-relative link so the page works at any depth. A file of the
theme is at `{{ prefix }}assets/<file>` on a site page and in the files
directory beside a `render --to html` page;
`{{ stylesheet | replace("style.css", "") }}<file>` reaches it on both.
A `document` page has an empty `prefix`, an empty `nav`, and no
breadcrumbs or neighbours. `note.html` receives `title`, `created`,
`tags` (`label`, `href`), `fields` (`key`, `html`), and `body`.

`note` is none on the front overview and on a tag, view, or group page
without a landing note, so test it with `{% if note %}`; `note is
defined` is true there too. A missing name, or an attribute of none,
prints nothing, but one step further fails: `{{ note.frontmatter.tagline }}`
on such a page stops the export with "the theme template `page.html` is
not usable: undefined value: `note.frontmatter` is undefined", naming the
line. `{{ debug() }}` prints everything a page receives, which is the
quickest way to see what a template has to work with.

The templates are minijinja 3
([ADR 0085](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0085-page-templates-render-with-minijinja-3.md))
with its built-in filters and tests, except `tojson` and `urlencode`,
and without the filters of minijinja-contrib. An unknown filter fails the
export naming the template and the line.

A note picks another template for its pages with `site.template` in its
frontmatter: `template: splash` renders it with `templates/splash.html`,
in a site export and in `render --to html` alike, which is how a landing
page gets a hero or an impressum drops the sidebar. A name the theme has
no template for is a warning and `page.html` is used. The templates are
read at export and never copied into the site; one that fails to parse or
to render fails the export naming it. A theme template whose own name
starts with `ntropy/` is refused.
