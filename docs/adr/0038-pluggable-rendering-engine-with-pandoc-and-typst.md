# 38. Pluggable rendering engine with pandoc and typst

Date: 2026-07-09

## Status

Accepted

Implements the engine behind the `render` command of
[ADR 0037](0037-render-command-surface.md). Interprets the headless-library
rule of
[ADR 0013](0013-library-binary-split-with-thiserror-and-anyhow.md);
testing follows
[ADR 0021](0021-testing-strategy-with-insta-across-all-layers.md).
`docs/design/rendering.md` carries the full model.

## Context

Rendering a note to PDF requires a typesetting toolchain that ntropy does
not contain, and further output formats are intended to follow, so the
engine abstraction must hold more than one implementation. The library is
headless per ADR 0013: it performs no terminal I/O, spawns no editor, and
runs no picker — and an engine that calls an external converter needs a
subprocess. The only external-tool precedent is the editor
(`$VISUAL`/`$EDITOR`, ADR 0015), which the binary spawns.

## Decision

### Formats and engines

Output selection is keyed by **format**, with a default **engine** per
format. The registry maps every format to the engines producing it; the
user names the format and optionally overrides the engine. An engine whose
external tools are missing is an error naming the tool to install, never a
silent substitution by another engine.

### Layering by capability injection

The library owns the whole engine: the `Renderer` trait, the shared
document preparation, the registry, and each engine's chain logic. Every
effect goes through a `RenderContext` capability handed in by the host,
with three v1 primitives: staging an intermediate file, running an
external tool, and the output path. The binary contributes the single
production context (temporary-directory staging, `std::process::Command`)
and the probe for an engine's external tools.

This reads ADR 0013's headless rule as: the library contains no spawn call
and no ambient effect, and requests effects only through injected
capabilities. The context grows a primitive only when a real engine needs
it.

### Shared preparation stays lossless

Engines consume a `PreparedDocument` carrying the note's facts in full:
id, storage path, title, tags, derived creation date, complete
frontmatter, the verbatim body, and the resolved link table. Preparation
never discards or formats; every lossy, output-shaped choice belongs to an
engine, so different engines can materialize the same facts differently.

### The v1 engine: pandoc with typst

`pdf` is produced by pandoc reading the staged body as GitHub-flavored
Markdown (`--from gfm`) with `--pdf-engine=typst`, which runs the `typst`
binary. Both tools are found via `PATH`; there are no configurable tool
paths in v1. The engine stages the body with frontmatter stripped and
resolved links replaced by the target's current title as emphasized text,
and passes title, date, and tags as `--metadata` arguments (tags as the
typeset `subtitle`; the `keywords` slot is unusable because pandoc's stock
typst template splices its value verbatim into typst code).

### Testing

Engine tests inject a fake `RenderContext` that records the staged files
and invocations and feeds back scripted outputs; the recorded sequence is
the `insta` snapshot. No v1 test executes pandoc or typst.

## Consequences

- ntropy gains its first runtime tool dependency beyond the editor:
  `render` works only where pandoc and typst are installed. Staging the
  intermediate render workspace promotes the `tempfile` crate from
  dev-dependencies into the runtime dependency tree; there are still no
  linked C libraries and distribution is unchanged.
- The exact pandoc invocation is pinned by snapshot without the tools
  installed, so the test suite runs everywhere it does today.
- Adding an engine or format is a library-only change against `Renderer`
  and the registry; the binary's context implementation already serves it.
- The library/binary boundary of ADR 0013 now has a stated capability
  interpretation, which future effectful features can follow.
