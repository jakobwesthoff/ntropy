---
title: Configuration
tags: [docs/notes]
site:
  order: 7
---
There is not much to configure, on purpose. This page lists what there is: the editor, the global default vault, and the per-vault file that holds views, document settings and site settings.

## Your editor

ntropy opens notes in `$VISUAL`, then `$EDITOR`. It won't guess a default: if neither is set, opening a note fails with a message saying so. Set one of them in your shell and ntropy uses it for `new`, `today`, and opening notes from the picker.

## Your default vault

`ntropy init --set-default` records a vault as the global fallback, used when nothing nearer resolves (see [Finding the vault](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md)). It lives in a small TOML file in your OS config directory, `~/.config/ntropy/config.toml` on Linux and `~/Library/Application Support/ntropy/config.toml` on macOS, holding a single line:

```toml
default_vault = "/Users/you/notes"
```

You'll rarely touch it by hand; `--set-default` writes it for you. Without that flag, `init` never touches the global config.

## Per-vault config

Each vault has its own `<vault>/.ntropy/config.toml`, so settings travel with the vault rather than your machine. Views are the main thing in it, one `[[view]]` table per view:

```toml
[[view]]
name = "by-tag"
field = "tags"

[[view]]
name = "by-status"
field = "status"
```

The `ntropy view` commands manage these entries for you; [Materialized views](01M28Q32CXG2ANTFTF9H1AKNS8-materialized-views.md) has the whole story.

The same file holds two optional tables. `[render]` sets the Typst theme and the paper size for rendered documents; see [Document themes](01M28Q32HRVNF9EBZTC69XDZ49-document-themes.md). `[site]` sets the site theme, the front page, the title, the language, whether pages list related notes, and the sidebar's root; it also takes `[[site.nav]]` tables for a hand-assembled sidebar and a free-form `[site.vars]` table that the site theme's templates read. [Exporting a website](01M28Q32JED0DJ32VP0B89P5F9-exporting-a-website.md) covers every key.

Every key of both tables is optional, and when ntropy writes the file it leaves out a table whose keys are all at their defaults.

Templates are not configuration. They are files under `<vault>/.ntropy/templates/`; see [Templates and daily notes](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md).
