# Skill: cover the site export, its look, and its structure

Requested by the user on 2026-09-11, to be done once the sidebar
definition (ADR 0056) is implemented: "update the skill in this project
to allow any[one] to properly understand and configure site exports look
and feel as well as structure".

The agent skill at `skills/ntropy/` (`SKILL.md` and `references/`) says
nothing about `ntropy site`: its command reference lists `render` but no
`site` row, and no reference file covers the export, its configuration,
its themes, or the frontmatter the sidebar reads. An agent following the
skill cannot build or shape a site today.

## What the skill has to cover

- **The command.** `ntropy site -o <dir> [query] [--theme <name>]
  [--force] [-p]` and `ntropy site theme init <name>`, with the
  non-interactive forms the skill's other rows use, the warnings the
  export prints and what each means (a referenced file missing or
  outside the vault, a fence language without a grammar, a link to a
  note the query left out), and `--strict`. Source:
  `docs/design/site-export.md`, "CLI surface".
- **Configuration.** The `[site]` table of `.ntropy/config.toml`:
  `theme`, `index`, `title`, `lang`, and from ADR 0056 `root` and the
  `[[site.nav]]` table with its item kinds (`note`, `tag`, `view` with
  optional `group`, `tags = true`, `label` with `items`), including that
  a nav table is the whole sidebar. Source: `docs/design/site-export.md`,
  "Configuration", and `docs/design/configuration.md`.
- **Structure from frontmatter.** The `site` table a note may carry:
  `order`, `label`, `hidden`, `index`, what each does, and the order rule
  within a group (ordered entries first, then notes newest first, then
  groups by label). A worked example of a documentation tree built from
  one tag subtree with landing notes, in the style of
  `references/writing-notes.md`. Source: ADR 0056.
- **Look and feel.** How a theme directory is laid out (`style.css`,
  `icons/*.svg`, `fonts/*`, anything else copied to `assets/`), that
  `site theme init` writes the built-in theme as the starting point,
  the custom properties a theme redefines and their names, the icon
  names the markup uses, and the page's markup anatomy a stylesheet
  targets. Source: the README's "Site themes" section, which is the
  contract a theme author writes against, and ADR 0055.
- **Workflows.** At least: "publish a subset of the vault as a docs
  site" (tag the notes, set the landing notes, configure the root or
  the nav table, export, read the warnings) and "restyle the site"
  (init a theme, edit the tokens, export with `--theme` to compare).
- **Golden rules.** The rules an agent must not break: never hand a
  reader a site whose export printed a link or file warning without
  saying so; `site` is a reserved frontmatter key; the nav table lists
  notes by ULID, never by title.

## Shape

A `references/site.md` file beside the existing references, a `site`
row in the command reference table of `SKILL.md`, and a pointer from the
skill's workflow section. The README's skill section
(`skills/ntropy/`) describes the skill's scope and may need the site
export added to it.

## Depends on

The implementation of ADR 0056 (the `site` frontmatter table, `[site]
root`, `[[site.nav]]`), so the skill documents what exists rather than
what is decided.
