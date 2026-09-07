# 45. Vault render themes

Date: 2026-09-07

## Status

Accepted

Realizes the theming hook the prelude of
[ADR 0040](0040-custom-typst-engine-with-own-markdown-emitter.md) was designed
around, adds a key to the `[render]` configuration section of
[ADR 0038](0038-pluggable-rendering-engine-with-pandoc-and-typst.md) and a flag
to the command surface of [ADR 0037](0037-render-command-surface.md). The
themes directory joins the reserved vault layout of
[ADR 0007](0007-vault-directory-layout.md).

## Context

Every rendered document looks the same. The engine's prelude was written as the
seam a theme would replace, but nothing reads a replacement: the look is
compiled into the binary.

Two needs drive this. A logo in the page header of every page, which in Typst is
a `header:` on `set page(...)` referencing an image file. And control over the
frontmatter metadata strip, since a document sent to a customer should not carry
internal tags or `status: draft`.

The assets such a theme needs are the obstacle. Typst confines file access to a
compile root, and the root was the note's own directory, so a logo anywhere else
was unreachable. Keeping one inside `all-notes/` contradicts that directory
holding notes and nothing else (ADR 0007).

## Decision

A theme is a Typst source file at `<vault>/.ntropy/themes/<name>.typ`, selected
by name.

### Selection

`[render] theme = "<name>"` in the vault's `config.toml` is the vault-wide
default: once set, a plain `ntropy render` renders themed, and so does a loop
over every note, with nothing theme-related on the command line.

`--theme <name>` overrides it for one invocation. The reserved name `default`
means the built-in look from either source, so a themed vault can still render
one document plain. Precedence is flag, then config, then built-in.

A name is one filename component. A name carrying a path separator or a `..`
is rejected by name, so the themes directory is the only place a theme is read
from.

### Composition

The emitted document is the prelude, then the theme file verbatim, then the
`#show: note.with(...)` application. Typst binds a name to the last `#let`
preceding its use, so a theme redefining `note`, `callout`, `notelink` or `task`
shadows the prelude's version, and a theme defining only some of them inherits
the rest. A theme therefore cannot omit something the body needs.

The three prelude helpers (`fmt-value`, `is-empty`, `callout-styles`) are
building blocks a theme may call. Redefining one does not change the built-in
`note` or `callout`: a Typst closure resolves the names it uses at the point it
is defined, which is above the theme. This was measured, not assumed.

### Assets

`typst compile` receives `--root <vault>` on every render, themed or not, so one
invocation shape covers both cases. A theme reaches a vault asset by a
root-absolute path, `image("/assets/logo.svg")`, and the asset stays outside
`all-notes/`.

Typst places a document read from stdin at the compile root rather than at the
working directory. Widening the root therefore moves where the note body's own
relative paths point, and `![](diagram.png)` beside a note would be looked for
at the vault root. The emitter resolves local image paths against the note's own
root-absolute directory, emitting `#image("/all-notes/diagram.png")`. A path
that climbs out of `all-notes/` resolves too, so a note can share a vault asset
with a theme; one that climbs out of the vault entirely is left as written for
the compiler to reject, rather than replaced by an invented path.

### Failure

A configured theme with no file, and a name that is not a single component, both
fail the render before the vault is scanned, naming the theme and the path
looked for. A theme that does not compile fails the render through the
compiler's own error, which cites the offending line of the emitted document.
There is no fall back to the built-in look, matching the registry's rule that an
unavailable engine is never silently substituted (ADR 0038).

### Contract

The four overridable functions and the signature of the template application are
the theme API, documented in `docs/design/typst-engine.md`. A snapshot test pins
the emitted `#show: note.with(...)` call, so a change to what a theme receives
fails the suite rather than silently breaking existing themes. There is no
version key in a theme file.

### Rejected alternatives

- **A theme replacing the prelude entirely:** every theme would restate the
  callout, task and note-link definitions, and one that omitted `task` would
  break any note with a checkbox at compile time.
- **A path instead of a name:** the config would carry a vault layout detail,
  and nothing would give themes a home.
- **Staging a temp compile directory, or embedding assets in the document:**
  both keep the sandbox at its current width, at the cost of ntropy having to
  know which files a theme's assets are.
- **Widening the root only for themed renders:** two invocation shapes, and a
  note with an image would render differently depending on whether the vault
  configures a theme.

## Consequences

- A vault renders in its own livery after one configuration line, for every
  note and with no per-invocation argument.
- A theme can reach any file inside the vault, and so can a note body; the
  compiler's sandbox is the vault rather than `all-notes/`.
- The emitted `typst` artifact addresses its assets from the vault root, so
  compiling one by hand takes `typst compile --root <vault>`. An artifact whose
  note and theme reference no files still compiles alone.
- `RenderOptions` is no longer `Copy`, since the theme name is a `String`.
- Theme loading is identical in an encrypted vault: encryption converts the
  notes in `all-notes/`, and `.ntropy/themes/` is plaintext either way, so a
  locked vault renders themed as soon as it can render at all.
