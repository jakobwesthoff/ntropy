Part of the ntropy skill. Read SKILL.md first, which holds the rules for every task; `ntropy skill` prints it.

# Rendering and showing a note

`render` turns one note into a document, `show` draws one note in the
terminal. Both take the selector `search` takes ([querying.md](querying.md),
or `ntropy skill querying`) and must resolve to exactly one note.

## Formats

`--to` picks the format:

| `--to` | Artifact (default name) | Needs |
|--------|-------------------------|-------|
| `pdf` (default) | a PDF, `<slug>.pdf`, compiled from the Typst document | the `typst` binary on `PATH` |
| `typst` | the Typst document that `pdf` compiles, `<slug>.typ` | nothing |
| `html` | a web page `<slug>.html` with a `<slug>_files/` directory beside it | nothing |
| `ansi` | the note drawn for a terminal, `<slug>.ansi`: ANSI escape sequences, 80 columns, GitHub alerts as callouts | nothing |

```bash
ntropy render -n -p 01ARZ3NDEKTSV4RRFFQ69G5FAV                  # ./<slug>.pdf
ntropy render -n -p --to html 01ARZ3NDEKTSV4RRFFQ69G5FAV         # ./<slug>.html and ./<slug>_files/
ntropy render -n -p --to ansi -o /tmp/note.ansi 01ARZ3NDEKTSV4RRFFQ69G5FAV
```

## Where the artifact goes

Without `-o` the artifact is `./<slug>.<ext>` in the **current directory**
(`.pdf`, `.typ`, `.html` or `.ansi`):
change into the directory it belongs in first, or an artifact lands in the
repository you are working in. `-p` prints the artifact's path and nothing
else, which is how a script captures it. `-o <path>` names the file.

`render` never overwrites silently: an existing artifact of any format is
refused, and so is a non-empty `<slug>_files/` for `html`, unless `--force`
replaces them. A default name is refused for a note whose file name holds a
control or bidi character or whose slug starts with `-`: run
`ntropy reconcile`, or pass `-o`.

## A set of linked notes

A link to another note becomes a link to `<target-slug>.pdf` (or `.html`),
the name `render` gives the target's own artifact by default. Render each
note from the same directory without `-o`, and the cross-references find
each other. Renaming an artifact breaks the links into it. A page rendered
to `html` links to `<slug>.html` beside it the same way.

## The theme

The look comes from the vault's config in `.ntropy/config.toml`; `--theme
<name>` overrides it for one render and `--theme default` forces ntropy's
built-in look.

| Format | Theme | Configured by |
|--------|-------|---------------|
| `pdf`, `typst` | a Typst file `.ntropy/themes/typst/<name>.typ` | `[render] theme` |
| `html` | a directory `.ntropy/themes/site/<name>/` | `[site] theme` |
| `ansi` | none; `--theme` with `--to ansi` is an error | |

A theme that is missing or broken fails the render and never falls back to
the built-in look. Writing a theme: [document-themes.md](document-themes.md),
or `ntropy skill document-themes`, and [site-themes.md](site-themes.md),
or `ntropy skill site-themes`.

`[render] paper` in `.ntropy/config.toml` sets the paper of `pdf` and
`typst`: `a3`, `a4` (the default), `a5`, `iso-b5`, `jis-b5`, `us-letter`,
`us-legal`, `us-tabloid`, `us-executive` or `us-oficio`. No flag sets it. A
theme whose `set page` does not pass `paper: paper` ignores the setting.

## One note as a standalone web page

```bash
ntropy render -n -p 01ARZ3NDEKTSV4RRFFQ69G5FAV --to html -o report.html
```

writes `report.html` and `report_files/` beside it, the way a browser saves
a page: the theme's stylesheet and fonts, the page script, the grammars the
page's code needs, and the images and files the note references. Move or
send the two together. The page keeps the header (titled after the note),
the light/dark switch, and the outline; it has no sidebar, search,
breadcrumbs, pager, or related notes, and its tags are plain text.

Warnings are the export's (missing or out-of-vault files, links to files the
export never copies, a fence language without a grammar), and `--strict`
makes them fail. Metadata left out of a linked folder is a `note:` line,
which `--strict` does not count. The whole vault as a site is
[site.md](site.md), or `ntropy skill site`.

## Encrypted vaults

`render` and `show` read the note, so the vault must be unlocked (a locked
vault fails with a message naming `ntropy unlock`). The artifact is plaintext:
write it outside the vault. Writing it inside an encrypted vault prints
`warning: ... is inside an encrypted vault; the artifact is not encrypted and
will sync as plaintext`, and that warning is not counted in a report's
"N warnings".

## Showing a note

`ntropy show -n <id>` prints the note's frontmatter, then its Markdown with
headings, emphasis, lists, tables, quotes, callouts and highlighted code
styled, laid out at the terminal's width (`--width <columns>` sets another).
Into a pipe it is plain text laid out at 80 columns, not the note's source:
read a note's exact text with `ntropy search -n -P <id>`. A note taller than
the screen opens in `$PAGER`, or `less -R`; `-n` never pages. `NO_COLOR`
leaves out the colours.

End of references/rendering.md.
