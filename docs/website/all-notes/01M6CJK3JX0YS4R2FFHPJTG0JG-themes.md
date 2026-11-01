---
title: Themes
tags: [docs/themes]
site:
  index: true
  label: Themes
  order: 5
---
A theme decides how a note looks once it leaves the vault. There are two
kinds, one for documents and one for web pages, and both live in the
vault's `.ntropy/themes/`.

## Two kinds of theme

| Kind | What it is | What it styles | Start with |
| :--- | :--- | :--- | :--- |
| Document theme | one Typst file in `.ntropy/themes/typst/` | PDFs, and the Typst document of `--to typst` | [Writing a document theme](01M6CAJG6C3WAZ2MD6W3EGBMKY-writing-a-document-theme.md), whose step 1 copies ntropy's own look into the file |
| Site theme | a directory in `.ntropy/themes/site/`: a stylesheet, fonts, icons and page templates | `ntropy site`, and the page of `render --to html` | `ntropy site theme init <name>`, then [Writing a site theme](01M6CBSVXZXEW713FS476VB76F-writing-a-site-theme.md) |

The `ansi` format takes no theme.
[Making a front page](01M6CBSVZBM74V17MGTHGWYECR-making-a-front-page.md)
continues the site guide. [Document themes](01M28Q32HRVNF9EBZTC69XDZ49-document-themes.md)
and [Site themes](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md) are the
references behind the guides.

## Selecting a theme

A vault names its themes in `.ntropy/config.toml`:

```toml
[render]
theme = "handout"   # .ntropy/themes/typst/handout.typ, for PDF and Typst output

[site]
theme = "harbor"    # .ntropy/themes/site/harbor/, for ntropy site and render --to html
```

Without a `theme` key, the built-in look applies, and `theme = "default"`
means the same. `--theme <name>` picks another theme for one command,
and `--theme default` the built-in look. A theme that is missing or
broken fails the command; ntropy never falls back to the built-in look
on its own.

## The theme test material

Two downloads show a theme the parts it styles.

The theme test note holds what a note body can contain: every heading
level, lists, quotes and callouts, tables, code, images, footnotes and
long words. It comes with a companion note, which it links to with a
heading anchor, and the two images it shows. Its archive adds the two
notes, tagged `theme-test`, to `all-notes/`, and the folder
`assets/theme-test/`:

```bash
curl -fsSL https://ntropy.westhoffswelt.de/downloads/theme-test-note.tar.gz | tar xz -C "$(ntropy info -p)"
```

In an encrypted vault, run `ntropy reconcile` afterwards, which encrypts
the two notes; see
[Encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md). To
remove them again:

```bash
ntropy delete -n -f 01M6CCEVBP701HH0VPBT6ZNAYZ
ntropy delete -n -f 01M6CCEVCDRQ5504MDH503KQT2
rm -r "$(ntropy info -p)/assets/theme-test"
```

The theme test vault is a small vault made to show a site theme every
kind of page and every part of a page the built-in theme draws. Its
front page, Welcome, is set by `[site] index` and lists what each note
shows:

- the theme test note, with nested quotes, callouts and tables, a
  frontmatter table with nested values, and an outline with all six
  heading levels, and its companion note;
- a landing note, "Site tour", that makes a group page with a listing,
  and three tour stops that give the sidebar order and the pager;
- "Tour detail", with a breadcrumb of three steps;
- "Raw HTML and remote images", with what only web pages show;
- a note whose title, tag, frontmatter key and frontmatter value are
  each one word wider than a phone screen, which shows how a theme wraps
  unbreakable text;
- a hidden note, with a page that no sidebar or listing names;
- the view `by-status` and the tags, which fill the sidebar.

Welcome also names what the vault cannot show: the generated front page
without `index`, a breadcrumb step without a link, which only a
`[[site.nav]]` table makes, an empty listing, and the parts the page's
scripts draw. Download it, copy a theme into it, and export it with that
theme:

```bash
curl -fsSL https://ntropy.westhoffswelt.de/downloads/theme-test-vault.tar.gz | tar xz
mkdir -p theme-test/.ntropy/themes/site
cp -R <vault>/.ntropy/themes/site/mine theme-test/.ntropy/themes/site/
ntropy --vault theme-test site -o theme-test-site --force --theme mine
```

Every [ntropy release](https://github.com/jakobwesthoff/ntropy/releases/latest)
also carries `theme-test-note.tar.gz` and `theme-test-vault.tar.gz`,
packed from the same commit as its binary.
