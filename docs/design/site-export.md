# Site export

The `site` command: how a vault becomes a static website. Decisions are
recorded in
[ADR 0046](../adr/0046-static-site-export-with-a-site-command-and-an-html-render-format.md)
(command and architecture),
[ADR 0054](../adr/0054-site-navigation-and-url-scheme.md) (pages and
navigation), [ADR 0047](../adr/0047-themes-directory-split-by-type.md)
and [ADR 0048](../adr/0048-site-themes-as-stylesheets-and-assets.md)
(themes). The converter is described in [html-engine.md](html-engine.md),
the browser-side code in [site-frontend.md](site-frontend.md), the page
templates' engine in
[ADR 0050](../adr/0050-page-templates-with-minijinja.md).

The export is a classic static-site generator. Rust pre-renders every
page; the browser-side scripts add search, the heading outline, the
color scheme switch, the drawer's keyboard handling, and syntax
highlighting, and never construct a page.
The output is a directory of files that any file host serves, and it
works when opened straight from disk: data the scripts need is embedded
in classic script files, every page is a real `.html` file, links are
relative, there is no client-side routing.

## CLI surface

    ntropy site -o <dir> [query] [--theme <name>] [--force] [-p]
    ntropy site theme init <name>

- `-o` names the output directory and is required. A non-empty directory
  is refused unless `--force` is given, in which case it is emptied before
  writing.
- The optional query is a DSL query ([query-and-search.md](query-and-search.md))
  restricting the exported notes; the default is every note. A link to a
  note outside the set renders as unresolved.
- `--theme` overrides the configured site theme for one invocation.
- `-p` prints the path of `index.html` as one line, so
  `open "$(ntropy site -o dir -p)"` composes. Without it a completion
  report names the directory, the page count, and the warnings.
- Scan warnings and export warnings print to stderr; `--strict` makes both
  kinds fail the exit code. The site is written either way. Export
  warnings are: a referenced file that is missing or lies outside the
  vault, a fence language without a grammar, a link to a note outside
  the exported set.
- An encrypted vault exports like a plaintext one; every note is
  decrypted in memory as for any read ([encryption.md](encryption.md)).
  When the output directory lies inside the vault, a warning says so, as
  `render` warns for an artifact. The site is plaintext, its directory
  names and page structure included.
- The command runs once and exits.

`site theme init <name>` writes the built-in theme's files to
`.ntropy/themes/site/<name>/` and refuses to overwrite an existing
directory.

## Configuration

The `[site]` table of the vault's `config.toml`:

| Key | Meaning | Default |
| :--- | :--- | :--- |
| `theme` | site theme name, resolved in `.ntropy/themes/site/` | the built-in theme |
| `index` | ULID of the note that becomes the front page | a generated overview |
| `title` | site title | the vault directory name |
| `lang` | the `html` `lang` attribute | `en` |

## Pages

One directory per node:

    index.html
    notes/<slug>.html
    tags/index.html
    tags/<a>/<b>/index.html
    views/<name>/index.html
    views/<name>/<group>/<sub>/index.html
    assets/                      the theme's files (style.css, icons/,
                                 fonts/, and whatever else it holds), the
                                 page script app.js, the search data, and
                                 under assets/grammars/ the highlighting
                                 grammars the site's code blocks need
    files/<vault path>           vault files and directories the notes
                                 reference

The `views/` and `tags/` prefixes keep a view named `notes` or `tags`
from colliding with the note pages. Every link in a page is relative to
that page, climbing to the site root with `../` per directory level, so
the site works from `file://` and from any path on a host.

**Front page.** The configured index note, or the generated overview:
site title, the note count with the span from the oldest to the newest
date, the ten newest notes, and for each section its top-level groups
with counts, each linking into its page.

**The `site` table.** A note's frontmatter may carry a `site` mapping
([ADR 0056](../adr/0056-sidebar-order-labels-landing-notes-and-a-nav-table.md)),
every key optional: `order`, an integer, the note's position among the
entries of every group holding it; `label`, a string, the name the
sidebar and the pager show instead of the title; `hidden`, a boolean,
which keeps the note out of the sidebar, every list, and the pager while
its page is still exported, linkable, and in the search data; and
`index`, a boolean, which makes the note the landing note of every group
it is a member of. The table is not shown as a frontmatter field on the
page. A `site` value that is no mapping, and a key of the wrong type,
are export warnings naming the note, and are ignored.

**Landing notes.** A group with a landing note shows, on its page, the
note's title, date, tags, fields, and body, then the child groups and
the listing; the note has no page of its own, and links to it go to the
group page, the first such group in sidebar order when it lands several.
The note's `label` names the group wherever the group is named, its
`order` places the group among the parent's entries, and the note is
not among the group's entries. The first member marked `index` is the
landing note.

**Note page.** `notes/<slug>.html`. Two notes may share a slug with
different ULIDs; only the colliding notes are named `<slug>-<tail>.html`,
the tail being the shortest ULID suffix of at least three characters that
separates them, the rule the view leaves use
([vault-layout-and-views.md](vault-layout-and-views.md)). The page shows
the title, the date, the tags each linking to its tag page, and every
other frontmatter field as key/value beneath, nested values included,
the `site` table excepted; hiding a field is a theme's job. A body that
opens with a level-one heading reading exactly the title has that
heading dropped
([html-engine.md](html-engine.md)). The page has an outline built from
the note's headings that highlights the current section while
scrolling, previous and next links, breadcrumbs, and, after the body,
the related notes: the notes sharing the most tags with it, a tag's
ancestors counted (`a/b` shares `a` with `a/c`), at most eight, ties
newest first, none when nothing is shared. There are no backlinks.

**Tag pages.** `tags/index.html` is the tag tree. A tag's page lists the
notes carrying the tag or any descendant tag, the sub-path rule of the
`tag:` predicate, and lists its child tags.

**View pages.** For each configured view an index of its groups, and a
page per group nested as the filesystem view nests its directories, with
the same grouping rules: a list-valued field places a note under each
value, `/` nests, values are normalized, notes without the field are
absent. A group's page lists its child groups and, like a tag page, the
notes of the group and of every group below it, after its landing note
when it has one. A view whose field is `tags` is no section and gets no
pages: the tag section is that view.

**Lists.** Wherever notes are listed (front page, tag and group pages,
related notes, search results) each note is one row: the date, the title
linking to the page, and the tags linking to their pages. Hidden notes
are in no list but the search results.

**Order.** The entries of a group are its notes and its child groups
together, in reading order: the entries with a `site.order` first,
ascending; then the notes without one, newest first, ULID descending, as
the CLI lists them; then the groups without one, by label. A note and a
group sharing an order keep the note first. The top-level groups of a
section follow the same rule among themselves. A group page lists its
descendants in reading order, a child's landing note before the child's
own entries, each note once. The front page's newest notes are newest
first.

**Sidebar.** Each configured view is a collapsible section whose groups
nest as above, each group's entries in reading order, notes and child
groups interleaved, a note shown by its label when it has one; a
section starts open only when it holds the current page, and so does
every group on the page's trail. The tag section lists the top-level
tags with their note counts, the one holding the current page marked;
the tree below lives on the tag pages. A note's first placement in
sidebar order defines its breadcrumb, statically, regardless of how the
reader arrived; its previous and next links follow the reading order
through the whole tree of that top-level section, across group
boundaries, a landing note before its group's entries, and name the
neighbours by their labels. On narrow screens the sidebar is a drawer
the header's menu button opens by targeting it, which needs no script;
Escape and following a link inside close it.

## Assets

The export copies the files inside the vault that exported notes
reference through images or links, plus the theme's files. A link to a
directory inside the vault copies the directory with its whole tree, so
the link resolves in the site as it does in the vault; an empty directory
is a warning. Local asset paths in note bodies are rewritten relative to
the page. A reference to a file outside the vault is a warning and the
file is not copied, as is a reference to a path that does not exist.
Remote images stay remote.

## Themes

The themes directory has one subdirectory per type:

    <vault>/.ntropy/themes/
      typst/<name>.typ     Typst themes ([rendering.md](rendering.md))
      site/<name>/         site themes

A site theme is a directory with one layout, the built-in theme and a
vault theme alike
([ADR 0055](../adr/0055-theme-directory-layout-with-fonts-icons-and-embedded-assets.md)):

    style.css        the entry point, required
    icons/*.svg      one icon per file
    fonts/*          files the stylesheet references with url(fonts/...)
    anything else    copied under assets/ as it is

Every page links `style.css` and `render --to html` inlines it; a
directory without it is not a theme. The HTML structure of every page is
ntropy's own, so a theme controls appearance, not markup. A theme
provides the palettes for both light and dark mode; the page follows the
system preference by default and remembers a manual choice in the
browser. The built-in theme keeps every color and typeface, and the
sizes that shape the pages (the reading column, the chrome's widths, the
gaps between blocks and sections, the radii, the shadows, the transition
length), in custom properties on the root element named for their job,
so a theme that wants only a palette or a wider column redefines those
and keeps the rest; the README lists them.

The icons are the theme's `icons/` directory. At export every
`<name>.svg` in it becomes a `<symbol id="icon-<name>">` of a hidden
sprite inlined into every page, and the markup shows an icon with
`<use href="#icon-<name>">`; inlining is what makes icons work over
`file://`. A vault theme's icons are layered by name over the built-in
ones, so one file replaces one icon and a theme without the directory
keeps them all. A file under `icons/` without an `<svg>` root fails the
export naming it. The built-in theme's icons are Lucide's, its fonts
Fraunces, Literata, DM Sans, and DM Mono, each committed with its
license.

Selection is `--theme`, then `[site] theme`, then the built-in theme. The
binary embeds exactly one built-in theme, every file under
`src/site/theme/`, and `site theme init` copies them out as the starting
point for a custom one. The README's theme section is the contract a
theme author writes against.

## Search data

The search runs in the browser over data embedded in the site: for every
exported note its id, title, page, creation date, tags, frontmatter, and
body, and for every page of the site itself (each section's index, every
tag page, every view group page) its kind, section, value, label, page,
and note count, as `assets/search-data.js`, a classic script that
assigns `window.__ntropySearch`. The notes come in the model's order, newest
first. Frontmatter travels as JSON with string keys only; a tagged YAML
value becomes `null`. Every `</` inside the JSON is written as `<\/`, so
a note whose body contains `</script>` cannot end the script that carries
it. A published site therefore carries each note twice, as its page and
inside the search data. The search itself is described in
[site-frontend.md](site-frontend.md).
