# 48. Site themes as stylesheets and assets

Date: 2026-09-11

## Status

Accepted

Amended 2026-09-11 by
[ADR 0058](0058-theme-templates-overriding-the-built-in-ones-by-name.md):
a theme may also hold `templates/`, overriding the built-in page
templates by name.

Amended 2026-09-11 by
[ADR 0055](0055-theme-directory-layout-with-fonts-icons-and-embedded-assets.md):
the directory layout a theme has (`style.css`, `icons/`, `fonts/`), the
icon sprite assembled at export from `icons/`, and how the built-in
theme is embedded.

The site-side counterpart of [ADR 0045](0045-vault-render-themes.md),
located per [ADR 0047](0047-themes-directory-split-by-type.md). The page
structure a theme styles comes from
[ADR 0050](0050-page-templates-with-minijinja.md).

## Context

The request asks that a user can choose between themes and create their
own. For the Typst engine a theme is a source file that redefines
functions of the prelude. HTML has two natural layers to hand over,
stylesheets and page templates, and handing over templates means
publishing a template language and a data model as a contract.

## Decision

### What a theme is

A site theme is a directory `<vault>/.ntropy/themes/site/<name>/` of
stylesheets and static assets. The HTML structure of every page is
ntropy's own, produced from templates embedded in the binary (ADR 0050).
A theme therefore controls appearance, not markup. Hiding frontmatter
fields, which the note page shows in full (ADR 0054), is a theme's job
through CSS.

A theme provides the palettes for both light and dark mode. The page
follows the system preference by default; a manual switch is remembered
in the browser.

### Selection

`[site] theme = "<name>"` in the vault's `config.toml` is the vault-wide
default; `ntropy site --theme <name>` overrides it for one invocation.
Without either, the built-in theme applies.

`render --to html` selects its theme the same way, from `[site] theme`
and `--theme`, resolved in `themes/site/`. The `pdf` and `typst` formats
keep `[render] theme` and `themes/typst/`. The meaning of `render`'s
`--theme` flag follows the format.

### Built-in theme

The binary embeds exactly one default theme. `ntropy site theme init
<name>` writes its files to `.ntropy/themes/site/<name>/` as the starting
point for a custom theme and refuses to overwrite an existing directory.

### Rejected alternatives

- **Templates plus stylesheets plus assets**, rendered by the template
  engine from the theme directory. A theme author would learn the
  template language and the data model handed to templates. Not part of
  this decision; the user named it as a possible later iteration.
- **A layered model** with built-in templates a theme may override
  individually.
- **Several embedded looks.**
- **A default that cannot be copied out**, with custom themes starting
  from documentation.
- **A flag on the export command** instead of `site theme init`, and
  copying the files by hand.
- **The built-in default always** for `render --to html`, ignoring vault
  themes, and **one `[render] theme` key** for both engines.

## Consequences

- A theme cannot change page layout or navigation markup; what a
  stylesheet can reach, plus the icons under `icons/` (ADR 0055), is the
  whole theme contract.
- The built-in theme's file layout is what `site theme init` writes, so
  changing it changes what new custom themes start from.
