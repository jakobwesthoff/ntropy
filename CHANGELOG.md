# Changelog

All notable changes to ntropy are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) for the `ntropy`
command-line program. The crate's library is internal and its API can
change in any release
([ADR 0075](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0075-version-numbers-cover-the-cli-not-the-library.md)).

## Unreleased

### Changed

- Two terms side by side in a query are an `and` that binds like the written
  one, so `ntropy search borrow checker` finds notes holding both words
  instead of failing with a syntax error, and `a b or c` is `(a and b) or c`.
  A mistyped operator, as in `tag:work nd status:done`, is no longer an
  error: `nd` is searched as a word
  ([ADR 0076](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0076-adjacent-terms-in-a-query-are-an-implicit-and.md)).
- `search`, `delete`, `render` and `site` read each query argument on its
  own. An argument with several words and no query syntax is a phrase, so
  `ntropy search "borrow checker"` finds those words in order, a line break
  included, and `"c++ tips"` is searched literally. An argument that is a
  query stays one, and beside other arguments it is grouped:
  `search tag:work and 'status:open or status:new'` now means
  `tag:work and (status:open or status:new)`. Blank arguments are ignored.
- Slugs replace every ASCII character outside `a-z`, `0-9` and `-` with a dash
  instead of dropping it, so `Node.js notes` is filed as
  `<ulid>-node-js-notes.md`, not `<ulid>-nodejs-notes.md`, and `a&b` no longer
  fuses into `ab`. The apostrophe is still dropped, so `Don't Panic` stays
  `dont-panic`. The first `reconcile` renames existing notes whose titles
  hold such characters and relinks the notes that point at them. The default
  `render` file name and the site's note page URL follow the new filename
  ([ADR 0077](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0077-punctuation-in-slugs-and-tags.md)).
- A bare query value may contain a dot, so `tag:milestone/0.13` and
  `version:1.5` work without quotes. Other punctuation such as `+` and `#`
  still needs quotes (`tag:"c++"`). A bare search word with a dot, such as
  `node.js`, used to be a syntax error and now matches the dot literally, in
  the CLI and in the site's search; a quoted string is still a pattern
  ([ADR 0077](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0077-punctuation-in-slugs-and-tags.md)).
- A field predicate finds its field with letter case folded, in the CLI and
  in the site's search, so `Status:done` finds a note with a `status` key
  instead of matching nothing. A note with keys that differ only in case
  (`Status` and `status`) matches when any of them holds the value. Values
  stay case-sensitive in the CLI. `tag` and `text` are reserved names in any
  letter case, so a frontmatter field with such a name still cannot be
  queried. In the site's search a number is compared as the same text, so
  `x:2.0` selects a note with `x: 2.0` and `x:2` no longer does, and
  `.inf` and `.nan` can be matched
  ([ADR 0078](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0078-field-predicates-ignore-name-case-and-compare-numbers-by-their-spelling.md)).

### Fixed

- Tags keep dots and other punctuation: `milestone/0.13` is listed and matched
  as written instead of becoming `milestone/013`, and `c++`, `c#` and
  `node.js` no longer collapse into `c` and `nodejs`. View directories and
  site pages use the slug of each tag segment (`by-tag/milestone/0-13/`,
  `tags/milestone/0-13/`); tags that share one slug, such as `node.js` and
  `node-js`, share a directory and a page. A `tag:` query, a site `root` or a
  navigation `tag` written in the old stripped form (`milestone/013`) no
  longer matches and has to name the tag as written. The language server
  inserts a tag that YAML would misread as a quoted string
  ([ADR 0077](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0077-punctuation-in-slugs-and-tags.md)).
- `delete` refuses an empty selector instead of matching every note.
- Links inside an inline code span that continues over a line break, and
  inside indented code blocks, are no longer treated as note links. `reconcile`
  leaves them alone, and rendering, site export and the language server skip
  them, as they already did for single-line spans and fenced blocks.
- An escaped backtick no longer opens a code span, and a backtick fence line
  whose info text contains a backtick no longer opens a fenced block that
  hides the rest of the note from link handling.
- A number or boolean in `title` or `tags` is read as its text. `title: 2026`
  was reported as a missing title and `tags: [2026, rust]` lost the tag `2026`;
  now they are the title "2026" and both tags. YAML does not keep how a number
  was written, so `1.50` reads as `1.5`; quote a value whose spelling matters.
  A list or mapping as the title, or as an entry of `tags`, now skips the note
  with a warning that names what was found and, for `tags`, the entry's
  position, where a tag entry was dropped without a word
  ([ADR 0081](https://github.com/jakobwesthoff/ntropy/blob/main/docs/adr/0081-numbers-and-booleans-in-titles-and-tags-read-as-text.md)).

## v2.3.0 - 2026-10-30

### Added

- `$NTROPY_CONFIG_DIR` names the directory that holds the global
  `config.toml`, in place of the OS config directory. An empty value counts
  as unset.
- The library re-exports `FsError`, the payload of `Error::Fs`, as
  `ntropy::FsError`, so code using the library can name it.
- The picker pages through long result lists. Ctrl-D and PageDown move the
  selection toward the best match, Ctrl-U and PageUp toward worse matches,
  by half a page and a full page. The selected row keeps its place on the
  screen while the list scrolls.

### Changed

- Version numbers and semantic versioning cover the `ntropy` CLI only. The
  crate's library is internal, and its API can change in any release; pin
  an exact version to build on it.
- The crate declares its minimum Rust version, 1.92, which Cargo checks
  before it builds. Its crates.io entry links the project page and lists
  keywords and categories.
- ntropy runs `$VISUAL` and `$EDITOR` through `/bin/sh`, so their value is
  shell code. A program path that contains spaces or other characters special
  to the shell needs quotes, such as
  `EDITOR="'/Applications/My Editor.app/Contents/MacOS/edit' -w"`. When the
  program cannot be found, the shell says so on the terminal, and ntropy
  reports that the editor exited with a non-zero status.
- View names are limited to lowercase letters, digits and single dashes,
  nested with `/` (`by-status`, `area/work`), with no part starting with a dot
  and no reserved name as the first part. `view add` refuses other names, such
  as `By_Tag`, `my.view` or `area work`, and suggests a valid one. A view
  already configured under such a name is skipped with a warning wherever
  views are synced, its old directory stays on disk and stays ignored by git,
  and `--strict` makes `reconcile` and `site` fail on it. Rename it with
  `ntropy view remove <name>` and `ntropy view add <new-name> --field <field>`,
  then delete the old directory. `vault encrypt` refuses to start until it is
  removed.
- Ctrl-U in the picker moves the selection half a page toward worse matches
  instead of clearing the query. Backspace and Ctrl-W still edit the
  query.
- A path that holds a control or bidi character is no longer printed by
  `-p`, `write` or Ctrl-Y in the picker: the command fails, or
  `search -n -p` leaves it out with a warning, prints the rest and exits
  non-zero. `render` without `-o` refuses to name its artifact after a note
  file whose name holds such a character or whose slug starts with `-`, and
  writes nothing. Run `ntropy reconcile` to give such a note its canonical
  name, or pass `-o` to name the artifact.
- `render --to pdf` runs `typst` in the directory ntropy runs in instead of
  the note's directory. A relative path in typst's own environment variables
  `TYPST_FONT_PATHS`, `TYPST_PACKAGE_PATH`, `TYPST_PACKAGE_CACHE_PATH` and
  `TYPST_CERT` now resolves against that directory. With `$PATH` unset,
  ntropy looks for `typst` in the C library's default path, not in the
  working directory.
- The library's `render::Invocation` has no `cwd` field.

### Fixed

- `reconcile`, and the filename realignment after an edit or a `write`, no
  longer rename a note over an existing file. Two files sharing a ULID and a
  title, such as a note copied by hand, were renamed to one name, and the
  file that held it lost its content; such copies now get their own ULID
  (below). A rename whose destination is still taken, for example by a file
  without frontmatter, is skipped with a warning, which `--strict` makes
  fail, and both files keep their content.
- Two note files that share a ULID, such as a note file copied by hand, are
  detected. The one with the shorter file name keeps the ULID, and every
  command that reads the vault skips the other with a warning naming it,
  which `--strict` makes fail. Links to that ULID resolve to the note that
  keeps it; before, they went to whichever file was read first. A full ULID
  passed to `edit` or `delete` still matches both files, and `new`, `today`
  and `write` report the copy, which their view refresh leaves out.
- `reconcile`, and the filename realignment after `edit`, `new`, `today` and
  `write`, give such a copy a ULID of its own. The new ULID keeps the
  original's time, so the note keeps its creation date, and every machine
  syncing the vault computes the same one. Each change is reported as
  `reassigned <old> -> <new>, its id stays with <keeper>`. In an encrypted
  vault, `reconcile` does the same for a plaintext note whose ULID is already
  taken, then encrypts it; before, such a note stayed unencrypted with a
  warning.
- `vault encrypt`, `vault decrypt` and `vault rekey` refuse to start while
  two note files share a ULID, or while a file sits where a converted note
  would be written. Before, encrypting such a vault failed halfway and left
  it mid-conversion, and decrypting replaced a plaintext note lying next to
  its encrypted twin. `vault encrypt` refuses before it writes the vault's
  key.
- A note whose view leaf name equaled another note's disambiguated name, such
  as a slug `review-123` beside two notes titled `Review` whose ULID tail was
  `123`, is no longer missing from the view. On a case-insensitive
  filesystem, a leaf name differing from another only in case, such as
  `review-fav` beside a tail `FAV`, no longer stops the view sync with "File
  exists". Leaf names in a group directory are now unique ignoring case; the
  site export's page names follow the same rule.
- `$VISUAL` and `$EDITOR` can carry arguments, such as `EDITOR="code -w"` or
  `EDITOR="emacsclient -t"`. Before, ntropy took the whole value as the
  editor's program name and could not start it.
- A file in `all-notes/` whose name has a multibyte character across its
  26th byte, such as `aaaaaaaaaaaaaaaaaaaaaaaaaé-xx.md`, no longer makes
  every command that scans the vault crash. It is skipped with a warning,
  like any other file whose name does not start with a ULID.
- Refreshing views no longer fails with "Directory not empty" when another
  ntropy process, such as the language server refreshing on save, adds a
  note to a view directory while the refresh removes that directory as
  empty.
- The query error for an unexpected character such as `+` no longer says
  that quoting the term searches it literally. In the CLI a quoted full-text
  term is a regular expression, and the message now says to escape its
  metacharacters with `\`. The exported site's search matches a quoted term
  as written, and its message says to escape metacharacters in a `text:`
  pattern.
- `ntropy init --set-default` writes the global config to a temporary file
  and renames it into place. If ntropy is killed or crashes during the
  write, the old `config.toml` stays intact instead of being left truncated.
  The rewritten file gets the default permissions of a new file.
- `ntropy info` lists only templates with a lowercase `.md` extension, so
  every name it lists loads with `ntropy new --template` on any filesystem.
  A `Meeting.MD` was listed as `Meeting`, which `new --template Meeting`
  could not find on a case-sensitive filesystem, and a directory named like
  a template was listed although it cannot be loaded.
- The picker no longer cuts an emoji apart at a column edge, such as
  keeping `#` without the rest of `#️⃣` or half of a flag.
- A query wider than the terminal no longer breaks the picker's frame. The
  prompt shows the end of the query, where you type, and the cursor stays
  on the screen.
- Growing the terminal while the picker shows the end of a long list no
  longer leaves blank rows above the list. The list fills the new rows.

### Security

- A view name in `.ntropy/config.toml` could point outside its directory,
  and the next view sync deleted every file in the directory it named, down
  through its subdirectories, that was not one of the view's links. A name
  such as `../Documents` or an absolute path reached outside the vault, and
  `.git` or `all-notes/x` reached into it. Every command that syncs views ran
  it, including `reconcile`, `new` and `today`, and `vault encrypt` deleted
  the named directory outright. A name with two `..` parts made `site` write
  pages outside its output directory, and a name with a line break added lines
  to `.gitignore`. View names are now validated before they become paths.
- A view's directory that was a symlink, or sat below one, as a cloned or
  synced tree can carry, led the view sync to empty whatever the link pointed
  to, and `vault encrypt` to delete the tree behind a linked parent. Such a
  view is now skipped with a warning, which `--strict` makes `reconcile` fail
  on, and `view add` and `vault encrypt` refuse it. A symlink
  swapped in while a sync is running is not caught.
- An image or link path in a note that started with `/` and climbed above
  the vault with `..` made `render --to html` and `site` copy files from
  outside the vault into the export, and with enough `..` write those
  copies outside the files directory or the output directory; a link to
  the directory above the vault copied all of it. Such a path now counts
  as outside the vault, like a relative one: the export warns and copies
  nothing. Both commands also refuse to write any file outside their
  output.
- A link in a note to a directory made `render --to html` and `site` copy
  the directory's whole tree, so a link to `/` put `.ntropy/`, with the
  configuration and an encrypted vault's identity file, and `.git/`, with
  the repository's history, into the output. Both now leave out note
  tools' configuration, version-control directories, sync and trash
  folders, credentials such as `.ssh` and `.env`, and file-manager files
  such as `.DS_Store`, at any depth, and every hidden entry of a linked
  directory; a site theme's files are copied by the same rules. A link
  naming such a file is a warning and keeps its text in the page.
  Metadata left out of a linked folder is a `note:` line, which `--strict`
  does not count.
- `render --to html` and `site` followed symlinks, so a file or folder in
  the vault that pointed elsewhere on the machine was copied into the
  output, and a site theme's stylesheet, icons and templates were read from
  wherever they pointed. Both now copy only what resolves inside the vault
  (for a theme: inside the theme or the vault), with a warning for the
  rest, whatever symlink leads there: an asset folder such as
  `assets -> ~/Pictures` and the images beside the notes of a symlinked
  `all-notes/` are no longer copied, so copy such files into the vault. A
  theme whose stylesheet, icon or template points outside fails to load.
- A link to `all-notes/`, to a view directory or to a note's file made
  `render --to html` and `site` copy notes the export leaves out: for
  `site`, the notes its query does not select; for `render --to html`, every
  note but the rendered one. Those are now left out with a warning, and a
  link to one keeps its text in the page; the exported notes are copied as
  before.
- Note titles, file names, link targets in note bodies, template names,
  view names, `.gitignore` entries and the vault's path reached the terminal
  unchanged. A vault you did not write could use them to send escape
  sequences that retitle the window, clear the screen or, on some
  terminals, write the clipboard, and a line break or tab in a title broke
  a table row. ntropy now shows control characters, and the bidirectional
  override and isolate characters that can reorder how a title reads,
  escaped as `\n`, `\t` or `\u{1b}` in tables, prompts, warnings, error
  messages, reports, the picker and the language server's completions and
  symbols.

## v2.2.0 - 2026-10-10

### Added

- Ctrl-Y in the `search` picker prints the selected note's path, as `-p`
  would, instead of opening it. It helps when you meant to pass `-p` and
  forgot. The stats line under the prompt hints at the key while the
  terminal is wide enough.

### Changed

- The picker splits the terminal width between its title and tag columns
  instead of cutting them at 48 and 32 columns. Titles and tag lists get an
  ellipsis only when the terminal is too narrow for them, and resizing the
  terminal lays the columns out again.
- The picker shows the selected note's ULID in the stats line under the
  prompt instead of at the end of every row.

### Fixed

- The picker measures emoji sequences the same way when it cuts a title or
  tag list and when it lines up the columns. A title with an emoji such as
  `#️⃣` no longer pushes the date out of line, and one with a joined emoji
  such as `🧑‍🤝‍🧑` is no longer cut short at the terminal edge.

## v2.1.1 - 2026-09-14

### Changed

- Dependencies updated to their latest compatible releases: `jiff` 0.2.37,
  `toml` 1.1.6.

### Fixed

- v2.0.0 and v2.1.0 still locked the yanked `chacha20` 0.10.1, as they were
  released from a line without the v1.12.1 lockfile refresh. The lockfile
  moves to 0.10.2.

## v2.1.0 - 2026-09-11

### Added

- A site theme may carry page templates: a `templates/<name>.html` in the
  theme directory replaces the built-in template of that name, and the
  built-in ones stay reachable as `ntropy/<name>` for a template that
  extends one and fills a block. The built-in `page.html` defines empty
  blocks at its seams (`head`, `header_nav`, `header_tools`,
  `before_content`, `after_content`, `footer`, `scripts`). The same
  templates render `render --to html`. A template that fails to parse or
  render fails the export naming it.
- Templates receive the page's `kind` (`front`, `note`, `group`,
  `document`), `path`, the `note` they render with its raw frontmatter,
  the sidebar as data (`nav`), and `vars`, a free-form `[site.vars]` table
  of the vault config.
- A note's `site.template` picks the theme template that renders its
  pages and its html artifact; a name the theme lacks is a warning and
  `page.html` is used.

### Changed

- `site theme init` writes the built-in templates into the new theme
  beside the stylesheet, icons, and fonts.
- A sidebar rooted at a tag or view group (`[site] root`, or a single
  `tag:` export query) shows the root's child groups as its sections
  instead of one section wrapping them; breadcrumbs start at the
  section. Every section's title now links to the section's page, in
  place of the "All groups" and "Overview" rows.

## v2.0.0 - 2026-09-11

### Added

- `ntropy site -o <dir> [query]` exports the vault as a static website:
  a page per note under `notes/`, a tag tree under `tags/` with a page per
  tag, a tree per configured view under `views/`, and a front page (the
  note named by `[site] index`, or a generated overview). Every page has a
  sidebar with the current trail opened, breadcrumbs, an outline of the
  note's headings, and previous/next links. Note links point at the
  target's page; vault files the notes reference are copied under
  `files/`, a linked directory with its whole tree. The site works when
  opened from disk. An optional query restricts the exported notes,
  `--force` empties a non-empty output directory, `-p` prints the front
  page's path, and `--strict` fails on export warnings.
- `ntropy site theme init <name>` copies the built-in site theme into
  `.ntropy/themes/site/<name>/` as the starting point for a custom one.
- A note's `site` frontmatter table shapes the site's navigation: `order`
  places the note among its group's entries, `label` names it in the
  sidebar and the pager, `hidden` keeps it out of the sidebar, the lists,
  and the pager while its page stays exported and searchable, and `index`
  makes it the landing note of its groups, whose pages then show the
  note in place of the listing (`listing: true` keeps the listing
  below it) and take the note's label and order. Group
  entries read in order: ordered ones first, then notes newest first,
  then groups by label; previous and next follow that reading order
  across group boundaries.
- The agent skill covers the website export: a `site` row and an
  html-aware `render` row, golden rules for the reserved `site`
  frontmatter key and for naming notes by ULID in the site config, a
  publish workflow, and two references, `site.md` (command, warnings,
  `[site]` and the nav table, the `site` frontmatter keys, the standalone
  page) and `site-themes.md` (layout, tokens, fonts, icons, markup). A test
  pins the theme reference's names to the built-in theme.
- The search palette's note results show their icon: the built-in theme
  was missing the `file-text` icon the palette uses.
- `ntropy render --to html` writes the site's page for one note as
  `<stem>.html` plus `<stem>_files/` beside it, holding the theme, the
  fonts, the page script, the grammars the page needs, and the images
  and files the note references, so highlighting, the scheme switch,
  the outline, the type, and the images all work as on the site. The
  page has no sidebar, search, breadcrumbs, pager, or related notes.
- `[site] root` roots the sidebar at a tag or view group page
  (`tags/docs`, `views/by-status/open`), and an export whose query is a
  single `tag:` predicate is rooted at that tag; `[[site.nav]]` tables
  assemble the sidebar by hand from notes, tag subtrees, views and their
  groups, the whole tag tree, and labelled groups of items, and are then
  the whole sidebar. Breadcrumbs follow the sidebar. `[site] related =
  false` drops the related notes from the note pages, and a note's
  `site.related` overrides that for its own page.
- Exported pages carry a scheme switch (system, light, dark) that
  remembers the choice in the browser, an outline that follows the
  reader's position, and code blocks highlighted by Shiki in the browser
  for 72 languages; a page loads only the grammars its code blocks need,
  and a fence language without a grammar is reported as an export
  warning.
- The exported site's look: Fraunces for titles, Literata for note
  bodies, DM Sans for the chrome, DM Mono for dates and counts, all
  shipped with the site; warm paper with amber-brown ink in light mode,
  warm black with gold in dark mode; Lucide icons in the chrome, on
  tags, and in callouts.
  Note lists are one row per note with the date, the title, and the
  tags as links. The sidebar keeps view sections collapsed away from
  their pages and lists the top-level tags with counts; on narrow
  screens it is a drawer. A note page links its tags to their pages,
  ends with the notes sharing the most tags with it, and drops a leading
  heading that repeats its title. A view over the `tags` field is
  skipped, since the tag section already is that view. `/` opens the
  search, Escape closes it.
- A site theme's layout: `style.css`, `icons/*.svg`, `fonts/`, and any
  other file, copied under `assets/`. The icons become a sprite every
  page inlines; a vault theme's icons replace or add to the built-in
  ones by file name. `site theme init` writes the complete built-in
  theme, fonts and icons included.
- Exported sites search in the browser, in a palette over the page that
  the header's button, `/`, or Ctrl+K opens. A word matches titles,
  tags, frontmatter values, and text; two words need both; `tag:`,
  `field:`, and `text:` narrow, `tag:` and `field:` partially, `text:`
  as the same regex as in the CLI; `and`, `or`, `not`, and parentheses
  combine. Results come grouped, tag and view pages before notes, with
  the matches marked and a line of context per note; the arrow keys and
  Enter open one. The search data is one script under `assets/`, loaded
  the first time the palette opens.
- `render --to html` writes a note as one self-contained web page: the
  site theme's stylesheet inlined, the title, tags, and remaining
  frontmatter as a header, the converted body with heading anchors, and
  note links pointing at `<slug>.html` beside the artifact. No external
  tool is involved.
- Site themes. A directory `<vault>/.ntropy/themes/site/<name>/` holding a
  `style.css` is a theme; `[site] theme` in the vault config selects it
  and `--theme` overrides it for the `html` format. A built-in theme with
  light and dark palettes ships in the binary.
- A `[site]` table in the vault config: `theme`, `index`, `title`, and
  `lang`, all optional.

### Changed

- `render` refuses an existing artifact of any format, and for `html` a
  non-empty `<stem>_files/` directory, unless `--force` replaces them;
  it used to overwrite the artifact in place. A script that re-renders
  into the same path needs the flag.
- Typst themes live in `<vault>/.ntropy/themes/typst/`, one subdirectory
  per theme type under `themes/`. A theme still at
  `.ntropy/themes/<name>.typ` fails the render with a message naming both
  the old and the new path; move the file to `themes/typst/` once.

## v1.12.1 - 2026-09-07

### Changed

- Dependencies updated to their latest compatible releases:
  `apple-native-keyring-store` 1.0.2, `clap` 4.6.6, `thiserror` 2.0.20,
  `toml` 1.1.5.

### Fixed

- `chacha20` 0.10.1, which reaches ntropy transitively through `ulid`, was
  yanked from crates.io, so building the previous release warned about it. The
  lockfile moves to 0.10.2.

## v1.12.0 - 2026-09-07

### Added

- Vault render themes. A Typst file in `<vault>/.ntropy/themes/` named by
  `[render] theme` in the vault config becomes the look of every rendered
  document: after that one line, a plain `ntropy render` and a loop over the
  whole vault both produce themed output with nothing theme-related on the
  command line. `--theme <name>` overrides it for one invocation and
  `--theme default` returns to the built-in look.
- A theme redefines only what it wants. Its source is emitted after the
  engine's prelude, so `note`, `callout`, `notelink` and `task` shadow the
  built-in versions and anything it leaves alone is inherited. The four
  signatures are documented as the theme API in `docs/design/typst-engine.md`
  and pinned by a snapshot test.
- Themes can use vault assets. `typst` now compiles with the vault as its
  root, so a theme reaches a logo or a font outside `all-notes/` with a
  root-absolute path such as `image("/assets/logo.svg")`, and `all-notes/` goes
  on holding notes and nothing else.

### Changed

- Local image paths in a note body are emitted as root-absolute paths
  (`#image("/all-notes/diagram.png")`). Typst places a document read from stdin
  at the compile root, so with the root widened to the vault a bare filename
  would otherwise be looked for in the wrong directory. A note can now also
  reach a shared vault asset with `../assets/logo.svg`.
- Compiling a `--to typst` artifact by hand takes `typst compile --root <vault>
  <file>.typ`, since the emitted document addresses its assets from the vault
  root. An artifact whose note and theme reference no files still compiles on
  its own.
- A missing or broken theme fails the render, naming the theme and the path it
  looked for. There is no fall back to the built-in look, so a document is
  never quietly produced in the wrong livery.

## v1.11.0 - 2026-09-07

### Changed

- A note link in a rendered document is now a real link, not just the target's
  styled title. It points at `<target-slug>.pdf`, which is the name `render`
  gives that target's own artifact by default, so a set of notes rendered
  without `-o` into one directory cross-references itself. The target is read
  from the vault at render time, so a slug that has drifted in the Markdown does
  not reach the artifact. The link is a plain relative reference: following it
  is the PDF viewer's behavior, and it finds nothing when the target was never
  rendered or was renamed.

## v1.10.0 - 2026-08-18

### Added

- `new --empty` creates the note file with no content in it, skipping the
  template step. ntropy still decides the identity, the location and the
  filename; the caller writes the frontmatter and body. This is for scripts and
  agents, which can then author a note in one write instead of parsing and
  rewriting around a stamped skeleton. `--template` is rejected alongside it,
  and until frontmatter is written the file is not a well-formed note, so a scan
  skips it with a warning.
- `ntropy write <id|filename|path>` replaces one note's content with text read
  from stdin. It names its target rather than searching for it, refuses text
  that is not a well-formed note before writing anything, and then realigns the
  filename and refreshes the views itself, so a written title cannot leave those
  stale. It never prompts and never opens a picker, terminal or not.
- Authoring a note in an encrypted vault no longer needs an editor, which a
  script cannot drive. `write` resolves its target from the directory listing
  and reads no note, so it works on a locked vault too, the same way `new`
  already did. Paired with `new --empty`, a caller composes a note and stores it
  without ever reading one back, identically in both kinds of vault.
- The agent skill teaches `--empty` and `write`: when to reach for them, the
  frontmatter and body contract a note written that way must satisfy, and the
  obligation to fill the file in the same step it is created.

### Fixed

- The agent skill no longer tells agents that `render` needs pandoc, which
  stopped being true in v1.7.0. It names `typst` as the only external tool, and
  documents `--to typst`, which needs none.

## v1.9.1 - 2026-08-06

### Changed

- Dependencies updated to their latest compatible releases: `anyhow` 1.0.104,
  `clap` 4.6.5, `ignore` 0.4.33, `jiff` 0.2.35, `libc` 0.2.189, `regex` 1.13.1,
  `serde` 1.0.229, `serde_json` 1.0.151, `thiserror` 2.0.19, `toml` 1.1.4.
- `ulid` updated to 3.0.0 and `lsp-server` to 0.10.0.

## v1.9.0 - 2026-08-06

### Added

- At-rest vault encryption. `ntropy init --encrypted` produces a vault whose
  notes are age ciphertext on disk, so whatever syncs the directory cannot read
  them; after a one-time `ntropy unlock` every command behaves as before.
  `ntropy lock` forgets the key again.
- `ntropy vault encrypt|decrypt|rekey|passphrase` convert an existing vault's
  storage, re-encrypt it to a fresh key, or change its passphrase. Each rewrites
  the whole vault and is safe to interrupt: every note is written and verified
  before anything is deleted, and `--resume` finishes a run that was. `rekey`
  keeps the vault's passphrase unless `--new-passphrase-file` names another.
- `search --print-content`/`-P` writes a note's text to stdout instead of
  opening the editor, resolving to exactly one note. Unlike `--print`, which
  names a file, it reads through the vault's cipher and so produces the same
  bytes whether or not the vault is encrypted.
- `--identity <path>` (or `$NTROPY_IDENTITY`) and `--passphrase-file <path>`
  supply a key and a passphrase without the OS credential store or a prompt,
  which is what makes an encrypted vault usable from a script. Neither writes
  to the credential store: only a passphrase you typed leaves the vault
  unlocked afterwards.
- The language server works against encrypted vaults with the same feature set.
  Following a link opens a decrypted, read-only copy outside the vault, since
  the note itself is ciphertext; editing still goes through `ntropy search`.
- `reconcile` adopts a plaintext note dropped into an encrypted vault by hand,
  encrypting it in place. It takes only files that already parse as notes and
  never overwrites an existing one; because encrypting needs no key, this works
  on a locked vault too.
- The `encryption` cargo feature, enabled by default, carries the cryptography
  and OS credential-store dependencies. Building with `--no-default-features`
  drops them; none links a C library, so distribution is unchanged either way.
  The command surface is compiled either way and reports the missing support at
  runtime.

### Changed

- Materialized views are unavailable in an encrypted vault, where a symlink tree
  would spell out the tag taxonomy in plaintext directory names. `view add` says
  so rather than accepting a definition that would produce nothing; `view list`
  and `view remove` keep working.
- `info` reports whether a vault is encrypted and whether it is unlocked. A
  locked vault says so instead of printing note statistics, which would be
  indistinguishable from an empty vault.
- Editing a note in an encrypted vault decrypts it to an owner-only file outside
  the vault, so a sync provider never sees the plaintext. Plaintext vaults are
  unaffected and still open the note itself, leaving editor swap and undo files
  exactly where they were. If the note changes underneath an open editor the
  write is refused rather than silently overwriting, and your version is kept at
  a path the error names.
- `render` warns when the artifact would land inside an encrypted vault: the
  output is plaintext by nature and would sync unencrypted. The render still
  proceeds and the exit code is unaffected.

## v1.8.0 - 2026-08-05

### Added

- `info --print`/`-p` reports the active vault's path alone instead of the
  full report, skipping the scan behind the statistics. The path is absolute
  whichever resolution rule matched, and an unresolvable vault exits non-zero
  with nothing on stdout.
- Shell integration under `contrib/shell/`, sourced from your rc file rather
  than installed: `ntropy.sh` (bash and zsh) defines `ncd`, which changes
  directory to the active vault and forwards its arguments to `ntropy`, so
  `ncd --vault ~/notes` works like any other command.

## v1.7.1 - 2026-07-10

### Changed

- Dependencies updated to their latest compatible releases: `anyhow` 1.0.103,
  `ignore` 0.4.28, `jiff` 0.2.32, `regex` 1.13.0.

### Fixed

- `render` to `pdf` no longer dies silently when the typesetting tool exits
  before reading the whole piped document; the broken pipe is tolerated and
  the tool's own exit status decides success.

## v1.7.0 - 2026-07-10

### Added

- `render --to typst` writes the note as a Typst document, converted by
  ntropy's own Markdown-to-Typst engine and needing no external tool on `PATH`.
- The render paper size is configurable per vault: a `[render]` section in
  `.ntropy/config.toml` with `paper = "us-letter"` (or `a3`, `a5`, `iso-b5`,
  `jis-b5`, `us-legal`, `us-tabloid`, `us-executive`, `us-oficio`; the
  default stays `a4`).

### Changed

- `render` to `pdf` now uses ntropy's own typst engine, which converts the
  note and compiles it with the external `typst` binary, so only `typst`
  must be installed and on `PATH`.
- Rendered notes have a redesigned default look: a4 paper, tags as chips in
  a subdued metadata block (fields with empty values are skipped), colored
  callouts per kind, code chips and panels, drawn task-list checkboxes,
  colored links with note links visually distinct, and lighter tables,
  quotes, and rules.

### Removed

- The pandoc render engine and with it the `--engine pandoc` choice; pandoc
  is no longer needed for anything.
- The content `init` seeds a vault with (the `README.md` and the `default` and
  `today` templates) now lives as real files under `src/vault/seed/`, embedded
  at compile time rather than written as Rust string literals (ADR 0039). A
  seeded vault is byte-for-byte unchanged.

## v1.6.1 - 2026-07-09

### Fixed

- A clippy lint new in Rust 1.97 (`unnecessary_sort_by`) failed the build
  under `-D warnings`; the pandoc engine's link-span sort now uses
  `sort_by_key`. No behavior change.

## v1.6.0 - 2026-07-09

### Added

- `render [id|query]` produces a document artifact from a single note. v1
  renders a PDF through pandoc with typst as the PDF engine, so both must be
  installed and on `PATH`. The selector follows the id-or-query rule of
  `search`/`delete` and, like `search`, may be omitted to fuzzy-pick from all
  notes; an ambiguous selector opens the picker interactively and errors with
  the candidate list under `-n`. `--to` selects the format
  (default `pdf`), `--engine` overrides the format's default engine,
  `--output`/`-o` names the artifact (default `./<slug>.pdf`, overwriting any
  existing file), and `--print`/`-p` prints the artifact path to stdout on
  success so `open "$(ntropy render -p ...)"` composes; without `-p` the
  command narrates the render and reports the artifact, engine, and size.
  Rendering is read-only with respect to the vault.
- `search` (and its aliases `list` and `edit`) now takes `--print`/`-p`: on a
  TTY the selected note's path is printed to stdout instead of opening the
  editor. A lone match prints directly; several matches open the picker and
  the chosen note's path prints; cancelling the picker exits non-zero so
  `p=$(ntropy search -p ...)` branches correctly. Non-interactively (`-n`)
  every match prints as one path per line, newest first.

### Changed

- **Breaking:** interactivity now keys off the controlling terminal instead of
  stdout. Redirecting output no longer switches ntropy into plain mode; only
  `-n` (or the absence of a terminal, as in cron and CI) does. The picker, the
  delete confirmation, and the editor all talk to the controlling terminal
  directly, so `ntropy search -p | pbcopy` opens the picker and pipes only the
  selected path. Scripts that piped or captured output without flags must now
  say so: `ntropy search ... | grep` needs `-n` for the table, and
  `path=$(ntropy new Title)` needs `-p` (or `-n`) to print the path instead of
  opening the editor.
- The `--no-edit` flag on `new` and `today` is renamed to `--print`, which was
  previously its alias, and gains the short form `-p`. `--no-edit` keeps
  working as a hidden alias, so existing scripts are unaffected.

## v1.5.0 - 2026-07-07

### Added

- `init` now seeds a `README.md` in the vault root that identifies the
  directory as an ntropy vault, links to <https://ntropy.westhoffswelt.de>,
  and shows how to install the CLI (`cargo install ntropy`), so anyone who
  discovers a vault knows how to access it. Like the templates, it is written
  only when absent: a re-init restores a deleted README but never overwrites
  an edited one. `README.md` is now a reserved name, so a view cannot clobber
  it.

## v1.4.0 - 2026-07-06

### Added

- An agent skill under `skills/ntropy/` that teaches LLM coding agents how to
  drive ntropy: a `SKILL.md` with the vault model and the non-interactive
  ground rules (always `-n`, `--no-edit` on `new`/`today`, `reconcile` after
  direct edits), plus reference docs on writing notes, querying, vaults, and
  views. Install it with `npx skills add jakobwesthoff/ntropy` or by copying
  the directory into an agent's skills folder; see the README's "Agent skill"
  section.

### Fixed

- Piping ntropy's output into a reader that exits early (e.g. `ntropy info |
  head -2`) no longer panics with `failed printing to stdout: Broken pipe (os
  error 32)`. ntropy now exits quietly on a closed stdout pipe, like
  conventional Unix tools (status 141).
- Titles with YAML-special characters (`Q3: Planning kickoff`, `[draft]
  roadmap`, `#hashtag first`) previously made `new` fail. Frontmatter
  placeholder substitution is now YAML-aware, quoting or escaping a
  substituted value only when its surrounding YAML needs it, so such titles
  work.
- A `new` whose template rendered an invalid note (e.g. one missing a `title`
  field) previously left the malformed file behind in `all-notes/`; every
  later command then warned about it until it was cleaned up by hand. The
  rendered note is now validated before anything is written, so a failed
  `new` leaves nothing behind.

## v1.3.0 - 2026-06-29

### Changed

- The plain tables (`search`/`list`, `tags`, `view list`) now render with
  space-aligned columns for every invocation, including piped and `-n` output.
  Columns are padded to their widest cell in Unicode display width with the last
  column left unpadded, so values that overflow a tab stop no longer push the
  following columns out of line. The tab-separated `awk`/`cut` positional format
  is retired (ADR 0033); `tail -n +2` still drops the header. Structured (JSON)
  output for machine consumers is planned.

### Fixed

- The interactive picker now fuzzy-searches the full note content. Titles and
  tag lists are clipped to fit their columns, but the matcher previously only
  saw the clipped text, so a long title's tail or a tag past the visible cap was
  unfindable. Matching now runs over the untruncated title, tags and date, while
  the columns stay width-capped; a match that lands in clipped-away text ranks
  the note without painting a stray highlight.

## 1.2.0 - 2026-06-27

### Added

- ntropy maintains a root `.gitignore` listing the derived materialized view
  directories, so committing a vault no longer tracks them. The entries stay in
  sync with the configured views through `init`, `reconcile`, and `view
  add`/`view remove`; lines you add to the file yourself are never touched.

### Changed

- Materialized views now refresh incrementally. After a mutation (and during
  `reconcile`), each view is diffed against its on-disk tree and only the links
  that actually changed are touched, instead of tearing down and regenerating
  every view tree from scratch. Unchanged links keep their identity, and a
  mutation's filesystem cost is proportional to what changed rather than to the
  whole vault. On a 3000-note vault with two views (Apple M1), this cuts a
  mutation or `reconcile` from roughly 820 ms to 135–150 ms (about 5–6×), with
  the saved time being almost entirely filesystem syscalls. `reconcile`'s
  summary now reads `synced N views` rather than `rebuilt N views`.
- `view remove` no longer deletes the view's directory. ntropy never deletes a
  directory: it prunes the view's `.gitignore` entry and leaves the now-stale
  directory in place, reporting it so you can delete it yourself. `reconcile`
  likewise prunes entries for views removed from config without touching their
  directories.

## 1.1.0 - 2026-06-26

### Fixed

- The interactive picker no longer panics on Ctrl-W when the query contains
  multi-byte whitespace (e.g. a non-breaking space): word deletion now advances
  by whole characters instead of bytes.
- Block-form tag completion no longer corrupts a tag containing a hyphen
  (`area/work-home`): the list-item dash is located by structure rather than by
  the last hyphen on the line, so accepting a suggestion replaces the whole tag.
- The picker now restores raw mode even when entering the alternate screen
  fails on startup, instead of leaving the shell without echo for the rest of
  the session.
- Link completion no longer drops a space from a display title that literally
  contains `) $0`; the snippet placeholder cleanup is confined to the snippet
  branch.
- Vault walk-up now reports a directory that looks like a vault but cannot be
  canonicalized as an error, instead of silently treating it as "no vault found"
  and falling through to the global default.
- The language server no longer points one character too far when a client
  sends an out-of-spec position inside a surrogate pair; it clamps to the start
  of the affected character.

### Changed

- Link completion now cooperates with editors that auto-close brackets: when the
  closing `]` was already inserted, accepting a completion overwrites it instead
  of leaving a duplicate.
- `reconcile` resolves link targets through an index rather than a linear scan
  per link, so refreshing links in large, well-linked vaults is markedly faster.

## 1.0.0 - 2026-06-26

First stable release: the user-facing interface is polished and initial
language-server support is added.

### Fixed

- `init` now honors the global `--vault` flag as the target when no positional
  path is given, instead of silently scaffolding the current directory. Passing
  both a path and `--vault` is rejected as a conflict.

### Added

- `info` command: reports the active vault and how it resolved, the global
  default vault, and vault statistics (note/tag/view/template counts, warnings,
  creation-date span, top tags, and template names).
- `today` command: opens today's note (titled by the date), creating it from the
  seeded `today` template on first use each day and reopening it afterward. `init`
  now also seeds `.ntropy/templates/today.md`.
- `new --template <name>` / `-t <name>` selects a template from
  `.ntropy/templates/<name>.md`; a missing named template is an error. Without
  the flag, `default.md` is used as before. See the README Templates section.
- `list` is now a visible alias for `search`.
- `reconcile` now prints a start line and a closing summary (notes scanned,
  files renamed, links relinked, views rebuilt, warnings). The summary always
  prints, so a no-op run is no longer silent.
- Inter-note links: a standard Markdown link whose target is the note filename,
  `[text](<ulid>-<slug>.md)`, is recognized by its leading 26-character ULID.
  `reconcile` refreshes stale link slugs to a note's current filename, keeping
  links resolvable and clickable after a rename. Links inside fenced or inline
  code are left untouched.
- Language server (`ntropy lsp`): an editor-agnostic LSP server over stdin/stdout
  that completes inter-note links (type `[`, pick a note by fuzzy-matching its
  title and tags) and frontmatter tags (flow and block forms, hierarchy-aware),
  and provides go-to-definition, document links, and workspace-symbol search
  across notes. It resolves a vault per open document and keeps an in-memory
  session cache refreshed by editor file-watch events. See
  [docs/design/language-server.md](docs/design/language-server.md).

### Changed

- Plain tab-separated tables (`search -n`, `tags`, `view list`) now start with an
  uppercase column header (docker-style). Strip it with `tail -n +2` if needed.
- The interactive fuzzy picker is now rendered in-house over `nucleo` and
  `crossterm` instead of `nucleo-picker`. It is bottom-anchored: the query
  prompt is framed by a blue divider line above and below it, with a dimmed
  stats line beneath (under the query text) showing the cursor's rank within the
  matches and the match/total counts, and the result list grows upward with the
  best match nearest the prompt. Rows are an aligned title/date/tags grid
  (widths measured in Unicode display columns) with the note's ULID shown dimmed
  and never matched. Matched characters are highlighted in yellow and the
  selected row in cyan with a `▌` bar, all from the terminal's own ANSI palette
  so the picker adapts to its light/dark theme. Type to filter; Ctrl-W (delete
  word), Ctrl-U (clear), and Up/Ctrl-P (toward worse matches) / Down/Ctrl-N
  (toward the best) navigation (ADR 0027).
- A single note reference (`date  title  [tags]  (id)`) is now used everywhere a
  note is named to a person: delete prompts and confirmations and the
  ambiguous-match list. The plain `search -n` table gained `date` and `tags`
  columns: `id<TAB>date<TAB>title<TAB>tags<TAB>path` (tags comma-joined). This
  changes the previous `id<TAB>title<TAB>path` format.
- `edit` is now a hidden alias of `search` rather than a separate command
  (ADR 0031). `search`/`list` accepts a full ULID or a query and is the single
  open entry point: on a TTY a single match opens directly in the editor and
  several open the picker, while piped/`-n` prints the plain table without ever
  opening an editor. A selector or listing that matches nothing now exits
  non-zero with the message on stderr.
- Full-text search (`text:` and bare terms) now uses the `regex` crate in place
  of the embedded ripgrep libraries (`grep-searcher`/`grep-regex`). Smart-case
  and matching are unchanged, except a pattern that explicitly spans a newline
  now matches across lines instead of being confined to one (ADR 0030).

## [0.9.0] - 2026-06-25

Initial release: a working, Unix-only (macOS, Linux) v1 of the ntropy CLI.

### Added

- Flat single-vault storage with canonical notes as
  `all-notes/<ulid>-<slug>.md`; identity is carried by the filename ULID and
  never stored in frontmatter.
- Permissive YAML frontmatter with recognized `title` (required) and `tags`,
  plus arbitrary preserved fields, and slash-separated hierarchical tags with
  German-aware slug/tag normalization.
- Stateless parallel scan of `all-notes/` that warns and skips malformed or
  badly-named notes; `--strict` promotes those warnings to errors.
- Query DSL (precedence `not` > `and` > `or`, parentheses) with `tag:` segment
  sub-path matching, `field:` equality and list membership, and regex `text:`
  full-text search with smart-case via the embedded ripgrep libraries.
- Materialized symlink views: group by any frontmatter field, list fan-out,
  `/`-nesting, normalized grouping values, `<date>-<slug>.md` leaves with
  trailing-ULID collision disambiguation, and relocatable relative link targets.
- `reconcile` to realign drifted filenames and rebuild views; views are also
  refreshed after every mutation.
- Note templates with `{{title}}`/`{{id}}`/`{{date}}`/`{{slug}}` substitution
  and a default template.
- Two-tier TOML configuration (global default vault, per-vault view
  definitions) and vault resolution order `--vault` > `$NTROPY_VAULT` > cwd
  walk-up (honoring a `.ntropy-vault` pointer) > global default.
- Commands: `init` (idempotent, `--set-default`), `new`
  (`--no-edit`/`--print`), `search`, `edit`, `delete` (`--force`),
  `reconcile`, `view list|add|remove`, and `tags`; with global `--vault`,
  `-n`/`--non-interactive`, and `--strict`.
- Interactive fuzzy picker on a TTY and `$VISUAL`/`$EDITOR` integration, with a
  plain newest-first `id<TAB>title<TAB>path` table when piped or run with `-n`.
- Derived dates rendered in the system-local timezone.
