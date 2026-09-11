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
light/dark toggle, and syntax highlighting, and never construct a page.
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
    assets/                      the theme's files, style.css among them,
                                 the page script app.js, and under
                                 assets/grammars/ the highlighting grammars
                                 the site's code blocks need
    files/<vault path>           vault files and directories the notes
                                 reference

The `views/` and `tags/` prefixes keep a view named `notes` or `tags`
from colliding with the note pages. Every link in a page is relative to
that page, climbing to the site root with `../` per directory level, so
the site works from `file://` and from any path on a host.

**Front page.** The configured index note, or the generated overview:
site title, note count, the newest notes, the top-level tags with counts,
the configured views, each linking into its index page.

**Note page.** `notes/<slug>.html`. Two notes may share a slug with
different ULIDs; only the colliding notes are named `<slug>-<tail>.html`,
the tail being the shortest ULID suffix of at least three characters that
separates them, the rule the view leaves use
([vault-layout-and-views.md](vault-layout-and-views.md)). The page shows
the title, tags as chips, and every other frontmatter field as key/value
beneath, nested values included; hiding a field is a theme's job. It has
an outline built from the note's headings that highlights the current
section while scrolling, previous and next links, breadcrumbs, and the
light/dark toggle. There are no backlinks.

**Tag pages.** `tags/index.html` is the tag tree. A tag's page lists the
notes carrying the tag or any descendant tag, the sub-path rule of the
`tag:` predicate, and lists its child tags.

**View pages.** For each configured view an index of its groups, and a
page per group nested as the filesystem view nests its directories, with
the same grouping rules: a list-valued field places a note under each
value, `/` nests, values are normalized, notes without the field are
absent. A group's page lists its child groups and, like a tag page, the
notes of the group and of every group below it.

**Order.** Notes inside a group, on a tag page, and on a group page are
sorted newest first, ULID descending, as the CLI lists them.

**Sidebar.** Each configured view and the tag hierarchy is a collapsible
section whose groups nest as above. There is no hand-curated order. A
note's first view and group in sidebar order define its previous and next
links and its breadcrumb, statically, regardless of how the reader
arrived.

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

A site theme is a directory of stylesheets and static assets. Its entry
point is `style.css`, which every page links and which `render --to html`
inlines; a directory without it is not a theme. The HTML structure of
every page is ntropy's own, so a theme controls appearance, not markup. A
theme provides the palettes for both light and dark mode; the page follows
the system preference by default and remembers a manual switch in the
browser. The built-in theme keeps every color in a custom property on the
root element, so a theme that wants only a palette redefines those and
keeps the rest.

Selection is `--theme`, then `[site] theme`, then the built-in theme. The
binary embeds exactly one built-in theme, and `site theme init` copies
its files out as the starting point for a custom one.

## Search data

The search runs in the browser over data embedded in the site: for every
exported note its id, title, page, creation date, tags, frontmatter, and
body, as `assets/search-data.js`, a classic script that assigns
`window.__ntropySearch`. The notes come in the model's order, newest
first. Frontmatter travels as JSON with string keys only; a tagged YAML
value becomes `null`. Every `</` inside the JSON is written as `<\/`, so
a note whose body contains `</script>` cannot end the script that carries
it. A published site therefore carries each note twice, as its page and
inside the search data. The search itself is described in
[site-frontend.md](site-frontend.md).
