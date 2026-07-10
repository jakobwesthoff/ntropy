# Rendering

The `render` command and its engines: how a note becomes an output artifact
such as a PDF. Decisions are recorded in
[ADR 0037](../adr/0037-render-command-surface.md),
[ADR 0038](../adr/0038-pluggable-rendering-engine-with-pandoc-and-typst.md), and
[ADR 0040](../adr/0040-custom-typst-engine-with-own-markdown-emitter.md);
selector semantics in [query-and-search.md](query-and-search.md), the overall
command surface in [cli.md](cli.md).

`render` produces one output artifact from a single note. Two formats ship:
`pdf`, the default, and `typst`, the emitted Typst document. Both come from
ntropy's own engine, which converts the note to Typst markup; the `typst` format
hands that markup out directly, while the `pdf` format compiles it with the
external `typst` binary. The typst engine's full model lives in
[typst-engine.md](typst-engine.md); this document covers the shared command
surface, preparation, and execution model.

## CLI surface

    ntropy render [id|query] [--to <format>] [--engine <name>] [-o <path>] [-p]

- The selector follows the id-or-query rule shared with `search` and
  `delete`: a full 26-char ULID resolves directly to that note, anything
  else runs as a DSL query. Like `search`, it is optional: omitted, every
  note feeds the picker for fuzzy selection.
- Like `delete`, `render` must resolve to exactly one note: several
  matches open the picker pre-filtered interactively; under `-n` an
  ambiguous selector errors with the candidate list (ADR 0025), and a bare
  invocation with more than one note asks for a selector. Interactivity
  keys off the controlling terminal (ADR 0036). A cancelled picker exits
  non-zero under `-p`, so `open "$(ntropy render -p ...)"` branches
  correctly, and is a successful no-op without it, like `delete`.
- `--to <format>` selects the output format, `pdf` (the default) or
  `typst`.
- `--engine <name>` overrides the format's default engine. Both shipped
  formats are produced by the typst engine; the flag exists so that
  invocations written today keep working when alternative engines
  arrive.
- `--output <path>` / `-o` names the artifact. The default is
  `./<slug>.<ext>` in the current working directory, where `<slug>` is the
  slug component of the note's filename and `<ext>` is the format's
  extension (`pdf` or `typ`). An existing file at the target is
  overwritten.
- `--print` / `-p` prints the artifact's path to stdout as one line on
  success, so `open "$(ntropy render -p ...)"` composes. Without it, a
  `Rendering <reference>...` line announces the work before the engine
  runs, and a completion report names the artifact, the format and engine
  that produced it, and its size:
  `Rendered quarterly-review.pdf (pdf via typst, 12.4 KiB)`.
- Scan warnings print to stderr and fail the command under `--strict`,
  matching `search`.
- `render` is read-only with respect to the vault: nothing was edited, so
  there is no filename realignment and no view refresh.

## Formats and engines

A **format** is the artifact kind the user asks for (`pdf`, `typst`). An
**engine** is an implementation able to produce one or more formats. The
registry maps
every format to the engines that produce it, one marked as the format's
default: `--to` picks the format, `--engine` optionally picks the engine
within it.

An engine whose external tools are missing is never silently substituted.
Different engines produce visibly different output, so an unavailable
engine is an error naming the tool to install, not a fallback to another
engine.

## Shared preparation

Before any engine runs, the library assembles a `PreparedDocument`: an
engine-agnostic, lossless view of the note and its vault context.

- The note's id, storage path, title, tags, and creation date (derived from
  the ULID, rendered in the system-local timezone, ADR 0010).
- The full frontmatter mapping: the lifted fields and every other field
  alike.
- The raw body, verbatim.
- The link table: every note-to-note link in the body (ADR 0028) with its
  span, display text, and target id, plus the target note's current title
  where the id resolves against the vault.

The guardrail for growing this type: a field must be a fact about the note
or its vault context, resolvable without knowing the output format.
Anything that discards information or shapes it for output belongs in an
engine. This keeps every engine free to choose its own materialization:
one engine may flatten links to styled text while a later one emits real
hyperlinks, both from the same resolved link table.

## Execution model

The library defines the engine abstraction; the binary contributes only the
ability to touch the outside world.

```rust
/// Everything an engine may ask the host to do while rendering.
pub trait RenderContext {
    /// Materialize an intermediate file in a render-scoped workspace.
    fn stage_file(&mut self, name: &str, contents: &[u8]) -> Result<PathBuf, RenderError>;
    /// Execute an external tool and return its captured output.
    fn run(&mut self, invocation: &Invocation) -> Result<ToolOutput, RenderError>;
    /// Write the final artifact directly, for an engine that produces the
    /// output bytes itself rather than delegating to an external tool.
    fn write_output(&mut self, contents: &[u8]) -> Result<(), RenderError>;
    /// Report a non-fatal degradation: content the engine could not carry
    /// faithfully into the artifact.
    fn warn(&mut self, message: &str);
    /// The path the final artifact must land at.
    fn output_path(&self) -> &Path;
}

pub trait Renderer {
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError>;
}
```

An `Invocation` carries the program and its argument vector, plus an optional
stdin payload and working directory, so an engine can feed a tool its work on
standard input and choose the directory it runs in (the typst `pdf` pipeline
uses both).

An engine's chain is ordinary sequential Rust inside `render`: stage files,
run tools, derive later steps from earlier output. Multi-step chains,
conditionals, and intermediate artifacts need no plan language, because the
logic never leaves the library; only the effects do.

The binary supplies the single production `RenderContext`: `stage_file`
backed by a temporary directory that is removed when the render ends, `run`
backed by `std::process::Command`, `write_output` backed by a filesystem
write to the artifact path, and `warn` printing to stderr. That one
implementation serves every engine, so adding an engine never touches the
binary. Probing for an engine's external tools is likewise the binary's job,
next to the spawn; the library defines the error the probe reports through.

Two placement details live with the production context because a tool may run
in a directory other than where the user stood:

- **Output-path absolutization.** The binary joins a relative `-o` path onto
  the process working directory before building the context, so a tool run in
  the note's own directory cannot land the artifact next to the note. The
  user-facing prints keep the path as the user gave it.
- **Program resolution in the parent's context.** The binary resolves a bare
  program name against `PATH` relative to the parent process cwd before
  spawning, because a spawn that first changed directory would otherwise
  resolve a relative `PATH` entry against the child's directory. A resolution
  failure is the missing-tool error naming what to install.

This is a capability reading of ADR 0013's headless rule: the library still
contains no spawn call and no ambient effect; it requests effects through a
context handed in by its host. Tests hand in a fake context instead (see
Testing).

The context grows a primitive only when a real engine needs it: the typst
engine emits its `typst`-format artifact through `write_output`, drives the
external compiler for `pdf` through `run`, and reports degraded content
through `warn`; `stage_file` serves engines whose external tool reads an
intermediate file from disk. A `warn` message prints to stderr and, under
`--strict`, counts toward a failing exit like a scan warning.

## The typst engine

Both formats are produced by ntropy's own engine, which converts the note
body to Typst markup with its own emitter and delegates only typesetting to
the `typst` binary. `typst` is the one external tool rendering needs, found
via `PATH`; there are no configurable tool paths, and a missing tool fails
the render with an error naming what to install. The engine's escaping
model, element mapping, document assembly, and asset resolution are
documented in [typst-engine.md](typst-engine.md).

## Testing

The engine seam is built for ADR 0021's snapshot style without the external
tools installed:

- Preparation: the `PreparedDocument` built from fixture notes is
  snapshot with `insta`.
- Engines: tests hand `render` a fake `RenderContext` that records every
  `stage_file`, `run`, `write_output`, and `warn` call and feeds back
  scripted outputs. The recorded sequence, staged contents and full argv
  included, is the snapshot, pinning the exact invocation without executing
  any tool. The typst engine's own escaping and emitter correctness is
  verified in-process against the `typst-syntax` parser (see
  [typst-engine.md](typst-engine.md)).
- CLI contract tests exercise the command end-to-end through a test-owned
  stub `typst` binary placed on `PATH`; the real typst is never executed by
  the standard suite. Contract tests pass `-n` or `--print` per ADR 0036.
- The kitchen-sink fixture (`tests/fixtures/kitchen-sink.md`) exercises
  every supported construct through the whole pipeline; the complete
  emitted document is pinned as one snapshot, so any emitter or prelude
  change surfaces as a single reviewable kitchen-sink diff.
- The roundtrip's final leg is a deliberate, opt-in exception to the
  no-external-tools rule: an `#[ignore]`d test runs the real `typst`
  binary over the kitchen-sink fixture via `just verify-render`, asserts
  the pdf compiles, and drops pdf/png/typ artifacts under
  `target/verify-render/` for optical inspection. It is not part of
  `just check`.

## Module layout

- `src/render/` (library): the document model and shared preparation, the
  format/engine registry, `Renderer`, `RenderContext`, `RenderError`, and
  the typst engine under `src/render/typst/`.
- `src/bin/ntropy/run/render.rs` (binary): `cmd_render` (selector
  resolution, picker on ambiguity, output-path defaulting) and the
  production `RenderContext`.

The selector plumbing is reused as-is: `ops::resolve_selection` and the
generic picker already serve `search` and `delete` unchanged.

## Configuration

Render options live in the `[render]` section of the vault's
`config.toml` and are typed enums, so a typo is a config parse error
naming the bad value before anything scans or renders. The options are
loaded by the binary and handed to `Registry::new`, which constructs the
engines with them; each engine decides how to honor a setting for the
formats it produces. One option exists: `paper` (default `a4`; also
`a3`, `a5`, `iso-b5`, `jis-b5`, `us-letter`, `us-legal`, `us-tabloid`,
`us-executive`, `us-oficio`), which the typst engine passes into the
emitted document's template application.

## Deferred

Not supported:

- rendering more than one note per invocation,
- styling and template control,
- a config surface for engine selection (per-format engine defaults,
  tool paths),
- a render action inside the search picker.
