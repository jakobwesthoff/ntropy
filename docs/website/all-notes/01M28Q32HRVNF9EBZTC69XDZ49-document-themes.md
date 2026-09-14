---
title: Document themes
tags: [docs/publish]
site:
  order: 2
---
A vault can render its PDFs in its own livery. A document theme is one
Typst file under `.ntropy/themes/typst/` that redefines what it wants and
inherits the rest. This page covers how to set one up, what a theme may
override, how it reaches assets in the vault, and the paper size. The
`html` format reads none of this; its look comes from a
[site theme](01M28Q32K56T4M52EGDDVYBSDX-site-themes.md).

## Setting up a theme

Drop a Typst file into `.ntropy/themes/typst/` and name it in the
`[render]` section:

```toml
# .ntropy/config.toml
[render]
theme = "corporate"
```

That is the whole setup. From then on every `ntropy render` in the vault
uses it, with nothing extra on the command line:

```bash
ntropy render 01j8za2…              # themed

# and so is every note in the vault
ntropy search -n | tail -n +2 | awk '{print $1}' |
  while read -r id; do ntropy render -n "$id"; done
```

`--theme <name>` overrides the configured theme for one render, and
`--theme default` goes back to ntropy's built-in look. `theme = "default"`
in the config means the same. A theme that is missing or does not
compile fails the render: a document is never quietly produced in the
wrong livery. A configured theme with no file fails before the vault is
scanned, with an error naming the path it looked for. A theme that does
not compile fails with typst's error, which cites the line of the
emitted document.

ntropy ships no named themes of its own beyond the built-in default.
Themes are the vault's own files.

## What a theme overrides

This theme puts a logo in the page header and drops the metadata strip,
so internal tags and `status: draft` never reach a customer:

```typst
// .ntropy/themes/typst/corporate.typ
#let note(title: none, frontmatter: (:), paper: "a4", body) = {
  set document(title: title) if title != none

  set page(
    paper: paper,
    margin: (x: 2.2cm, top: 3.4cm, bottom: 2.4cm),
    header: {
      align(right, image("/assets/logo.svg", width: 3.2cm))
      v(-0.4em)
      line(length: 100%, stroke: 0.6pt + rgb("#2dd4bf"))
    },
  )
  set text(size: 11pt)

  if title != none {
    text(size: 1.6em, weight: "bold", title)
    v(0.8em)
  }

  // No frontmatter strip: nothing but the body below the title.
  body
}
```

Four functions are yours to override; a theme defining only `note` keeps
the built-in look for the others:

| Function | Signature | Renders |
| :--- | :--- | :--- |
| `note` | `note(title: none, frontmatter: (:), paper: "a4", body)` | the whole document |
| `callout` | `callout(kind: "note", body)` | a `> [!NOTE]` admonition |
| `notelink` | `notelink(body)` | a link to another note |
| `task` | `task(done: false)` | a task-list checkbox |

The theme's source is spliced into the emitted document after ntropy's
own definitions and before the document applies `note`. Typst binds a
name to the last definition above its use, which is why a theme's
`note` wins and a function the theme leaves alone keeps the built-in
one. `note` receives the complete frontmatter mapping as typed Typst
values, so which fields to show or hide is the theme's decision. The
built-in definitions also include the helpers `fmt-value`, `is-empty`,
and `callout-styles`, which a theme may call; redefining one of them
does not change the built-in `note` or `callout`.

## Assets and the vault root

The logo above lives at `<vault>/assets/logo.svg`, outside `all-notes/`,
which holds notes and nothing else. Paths starting with `/` are relative
to the vault root, which is what the compiler is given access to.

> [!NOTE]
> Because a theme's assets are addressed from the vault root, compiling a
> `--to typst` artifact by hand takes `typst compile --root <vault> note.typ`.

## Paper size

Rendering defaults to a4. A `[render]` section in the vault's
`.ntropy/config.toml` picks a different format:

```toml
[render]
paper = "us-letter"
```

Supported values: `a3`, `a4`, `a5`, `iso-b5`, `jis-b5`, `us-letter`,
`us-legal`, `us-tabloid`, `us-executive`, `us-oficio`. An unknown value
is a config error naming the bad name, reported before anything renders.
The value reaches the theme as the `paper` argument of `note`.
