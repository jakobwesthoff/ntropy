# 58. Theme templates overriding the built-in ones by name

Date: 2026-09-11

## Status

Accepted

Amends [ADR 0048](0048-site-themes-as-stylesheets-and-assets.md), whose
theme was stylesheets and assets over markup that is ntropy's own;
[ADR 0050](0050-page-templates-with-minijinja.md), whose templates were
reachable only inside the binary; and
[ADR 0056](0056-sidebar-order-labels-landing-notes-and-a-nav-table.md),
whose `site` frontmatter table gains a key. Extends the directory layout
of
[ADR 0055](0055-theme-directory-layout-with-fonts-icons-and-embedded-assets.md).

## Context

The project page of ntropy is to become an export of a documentation
vault (research log, `docs/research/project-page/`). Its entry page
needs a hero, its pages a footer and a link to an impressum, and a
stylesheet cannot produce markup. The user decided to build the template
override, deferred in ADR 0048, first (research log, Q91 to Q98).

## Decision

### Templates are part of a theme

A theme directory may hold `templates/*.html`. The built-in theme's
templates live under `src/site/theme/templates/` and are embedded with
its other files, so `site theme init` writes them into a new theme as
it writes the stylesheet, the icons, and the fonts. The `templates/`
directory is the one theme directory not copied under the site's
`assets/` nor beside a rendered artifact.

### Override by name, built-ins reachable

A theme's `templates/<name>.html` replaces the built-in template of that
name. Every built-in template is also registered as
`ntropy/<name>.html`, so a theme template extends or includes a built-in
one and overrides single blocks. A theme file whose name starts with
`ntropy/` fails the theme load naming it. A template that fails to parse
fails the load; one that fails to render fails the export.

### Selection

A page renders with `page.html` unless the note it is rendered from
names another template with `site.template = "<name>"` in its
frontmatter, which selects the theme's `templates/<name>.html`. A name
the theme has no template for is an export warning, `--strict` fails on
it, and the page uses `page.html`. Pages rendered from no note (a group
page without a landing note, a section index, the generated overview)
always use `page.html`. `note.html`, the fragment of a note's header and
body, is overridable by name as well.

### The kind

Every template receives `kind`: `front` for the front page, `note` for a
note's own page, `group` for a tag, view, or group page including one a
landing note renders, and `document` for the `render --to html`
artifact. It replaces the `document` flag of ADR 0057.

### Context

Beside the fields of ADR 0050 (`lang`, `site_title`, `title`, `prefix`,
`stylesheet`, `scripts`, `icons`, `sidebar`, `breadcrumbs`, `outline`,
`prev`, `next`, `body`), a template receives `path`, the page's
site-relative path; `note`, the note a page is rendered from with its
id, title, creation date, tags, and the raw frontmatter mapping, or
undefined for a page rendered from no note; `vars`, the `[site.vars]`
table of the vault config, a free-form table the built-in templates do
not read; and `nav`, the sidebar as data (sections with their label,
page, and items; items with their label, page, whether they hold the
current page, and their own items) beside the rendered `sidebar`.

### Blocks

The built-in `page.html` defines empty blocks at its seams: `head`
(extra tags in the head), `header_nav` (after the site name),
`header_tools` (beside the search button and the scheme switch),
`before_content` and `after_content` (inside `main`, around the
content), `footer` (after the layout), and `scripts` (at the end of
the body); `title`, `body_class`, and `body` come from `base.html`.

### Rejected alternatives

- **Whole files only**, with the built-in gone for an overridden name;
  and **blocks only**, with no whole-file replacement.
- **Per-kind template files** (`front.html`, `group.html`) resolved by
  the exporter, alone or beside `site.template`.
- **A visible built-in footer**, and **no footer block**.
- **Header and footer blocks only.**

## Consequences

- The template context is a contract theme authors write against; it is
  documented in the README and the skill's theme reference, and a test
  pins the names the reference gives to the code.
- A theme can now break the page structure the built-in stylesheet
  targets; the classes it keeps are the theme author's responsibility.
