# 57. The html artifact as a page with a files directory

Date: 2026-09-11

## Status

Accepted

Amended 2026-09-11 by
[ADR 0058](0058-theme-templates-overriding-the-built-in-ones-by-name.md):
the `document` flag of the page template becomes the kind `document`.

Amends [ADR 0046](0046-static-site-export-with-a-site-command-and-an-html-render-format.md),
whose `html` format produced one self-contained file, and
[ADR 0051](0051-browser-side-code-in-typescript-with-committed-build.md),
whose build produced one page script. The user's answers are recorded
in `docs/research/web-export/decisions.md`, Q87 to Q90. The overwrite
rule extends the `render` command of
[ADR 0037](0037-render-command-surface.md).

## Context

`render --to html` inlined the theme's stylesheet and icon sprite and
nothing else. The page had no syntax highlighting, no scheme switch,
and no outline, since it loaded no script; its fonts fell back to
system faces, since the stylesheet's font files were not beside it; and
its image paths were written as the author typed them, so they broke
unless the artifact was placed beside the note. Inlining all of that
would make a note with two screenshots a file of a megabyte or more.
Browsers save a page as `Name.html` plus `Name_files/`, and Windows
Explorer moves and deletes that pair together.

## Decision

### Shape

`render --to html` writes `<stem>.html` and `<stem>_files/` beside it,
where `<stem>` is the artifact's file name without its extension, so
`-o report.html` writes `report_files/`. The directory mirrors the
site's `assets/` layout: `style.css` and the rest of the theme's files
except `icons/`, `app.js`, the grammars the page's code blocks need
under `grammars/`, and the images and files the note references under
`files/<vault path>`, resolved and copied as the site does, a missing
or out-of-vault file being a warning. The icon sprite stays inline, as
on a site page.

### Chrome

The page is the site's page without the parts that need a site: the
header with the note's title where a site page shows the site name, the
scheme switch, and the outline; no sidebar, search, breadcrumbs, pager,
or related notes. Tags stay plain text. The `site` frontmatter table is
hidden and has no effect.

### Overwriting

`render` refuses an existing artifact of any format, and a non-empty
`<stem>_files/`, unless `--force` is given, which replaces both.

### The bundle

The frontend build produces two scripts: `app.js` with the scheme
switch, the drawer, the outline, and the highlighter, and `search.js`
with the search palette. A site page loads both; the artifact loads
`app.js`. Nothing is built twice.

### Rejected alternatives

- **One self-contained file** with fonts and images as data URLs and
  the scripts inline; **the scripts inline and the fonts as
  fallbacks**; **leaving the artifact as it was**.
- **`<stem>_assets/`** as the directory's name.
- **The site title in the header**; **the switch and outline without a
  header**; **the body alone**.
- **Replacing the directory without asking**, and **refusing only for
  the html format** while `pdf` and `typst` overwrite.
- **Loading `app.js` whole** into the artifact, search included.
