# 47. Themes directory split by type

Date: 2026-09-11

## Status

Accepted

Amends the theme location of [ADR 0045](0045-vault-render-themes.md).
Site themes are [ADR 0048](0048-site-themes-as-stylesheets-and-assets.md).

## Context

ADR 0045 put a Typst theme at `<vault>/.ntropy/themes/<name>.typ`. A site
theme is a directory of stylesheets and assets, not a single file. Putting
`<name>/` beside `<name>.typ` mixes two kinds of theme in one directory
with nothing but the file extension telling them apart.

## Decision

The themes directory has one subdirectory per theme type:

    <vault>/.ntropy/themes/
      typst/<name>.typ     Typst themes (ADR 0045)
      site/<name>/         site themes (ADR 0048)

Existing Typst themes move into `typst/`. There is no fallback to the old
location. A configured Typst theme that is found at `themes/<name>.typ`
but not at `themes/typst/<name>.typ` fails the render with a message that
names the old location and the new one. The change is documented in the
changelog.

### Rejected alternatives

- **Moving with a deprecation fallback** that keeps reading
  `themes/<name>.typ` for a while.
- **Keeping Typst themes flat** and giving only site themes a type
  directory.
- **A separate `site-themes/` folder** beside `themes/`.
- **One theme name selecting both** `x.typ` and `x/`.

## Consequences

- A vault with a Typst theme needs its file moved once; a render that
  forgets says where the file is looked for.
- `Layout::theme_file` and the theme loader of ADR 0045 change their
  path; the selection rules of ADR 0045 are unchanged.
