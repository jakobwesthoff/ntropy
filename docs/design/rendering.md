# Rendering

The `render` command and its engines: how a note becomes an output artifact
such as a PDF. Decisions are recorded in
[ADR 0037](../adr/0037-render-command-surface.md),
[ADR 0038](../adr/0038-pluggable-rendering-engine-with-pandoc-and-typst.md), and
[ADR 0040](../adr/0040-custom-typst-engine-with-own-markdown-emitter.md);
selector semantics in [query-and-search.md](query-and-search.md), the overall
command surface in [cli.md](cli.md).

`render` produces one output artifact from a single note. Three formats ship:
`pdf`, the default, and `typst`, the emitted Typst document, both from
ntropy's typst engine, which converts the note to Typst markup, hands it out
directly for `typst`, and compiles it with the external `typst` binary for
`pdf`; and `html`, a self-contained web page from the html engine, which
converts the same note through the shared Markdown walk and inlines the site
theme's stylesheet. The engines' models live in
[typst-engine.md](typst-engine.md) and [html-engine.md](html-engine.md); this
document covers the shared command surface, preparation, and execution
model.

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
- `--to <format>` selects the output format, `pdf` (the default),
  `typst`, or `html`.
- `--engine <name>` overrides the format's default engine. Both shipped
  formats are produced by the typst engine; the flag exists so that
  invocations written today keep working when alternative engines
  arrive.
- `--output <path>` / `-o` names the artifact. The default is
  `./<slug>.<ext>` in the current working directory, where `<slug>` is the
  slug component of the note's filename and `<ext>` is the format's
  extension (`pdf`, `typ`, or `html`). The `html` format also writes
  `<stem>_files/` beside the artifact
  ([html-engine.md](html-engine.md)). An existing artifact, or a
  non-empty files directory, is refused unless `--force` is given,
  which replaces both.
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

### Encrypted vaults

A note in an encrypted vault is decrypted in memory like any other read, so the
engines see the same `PreparedDocument` either way and know nothing about
encryption ([encryption.md](encryption.md)). Two things do differ.

The render workspace is staged in the runtime directory — `$XDG_RUNTIME_DIR`
where one exists, otherwise the system temp directory — rather than wherever a
temporary directory would otherwise land. Nothing an engine works on is written
inside the vault, so a sync provider never sees a note's body in the clear.

The artifact itself is plaintext by nature. When the resolved output path lies
inside an encrypted vault the render proceeds and prints a one-line warning to
stderr. The common accident is a shell whose working directory happens to be
the vault, where the default `./<name>.pdf` would drop a readable copy of the
note straight into the synced directory. The warning is advisory and does not
affect the exit code: it concerns where the user chose to put the artifact, not
anything the engine failed to carry.

The default artifact name comes from the note's slug, which in an encrypted
vault is derived from the decrypted title rather than read from the filename.
The result is the name a plaintext vault would have produced.

## Formats and engines

A **format** is the artifact kind the user asks for (`pdf`, `typst`, `html`). An
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
  and slug where the id resolves against the vault.

The guardrail for growing this type: a field must be a fact about the note
or its vault context, resolvable without knowing the output format.
Anything that discards information or shapes it for output belongs in an
engine. This keeps every engine free to choose its own materialization:
one engine may flatten links to styled text while another emits real
hyperlinks, both from the same resolved link table. The typst engine does
the latter, pointing each link at the target note's own artifact
(ADR 0044).

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
  format/engine registry, `Renderer`, `RenderContext`, `RenderError`, the
  shared Markdown walk and its output trait under `src/render/markdown/`,
  the typst engine under `src/render/typst/`, and the HTML emitter under
  `src/render/html/`.
- `src/bin/ntropy/run/render.rs` (binary): `cmd_render` (selector
  resolution, picker on ambiguity, output-path defaulting) and the
  production `RenderContext`.

The selector plumbing is reused as-is: `ops::resolve_selection` and the
generic picker already serve `search` and `delete` unchanged.

## Configuration

Render options live in the `[render]` section of the vault's
`config.toml`. The options are loaded by the binary and handed to
`Registry::new`, which constructs the engines with them; each engine
decides how to honor a setting for the formats it produces.

- `paper` (default `a4`; also `a3`, `a5`, `iso-b5`, `jis-b5`,
  `us-letter`, `us-legal`, `us-tabloid`, `us-executive`, `us-oficio`) is
  a typed enum, so a typo is a config parse error naming the bad value
  before anything scans or renders. The typst engine passes it into the
  emitted document's template application.
- `theme` names a Typst file in `<vault>/.ntropy/themes/typst/` and is the
  vault-wide default look (ADR 0045). It is a free-form string rather
  than an enum, because the legal values are whatever files the vault
  holds; a name resolving to no file is reported when the theme loads,
  naming the path. `--theme` overrides it per invocation and the reserved
  name `default` selects the built-in look from either source.

The `html` format reads none of this: its look comes from `[site] theme`,
a directory in `<vault>/.ntropy/themes/site/`, and its page language from
`[site] lang` ([site-export.md](site-export.md)). `--theme` names whichever
kind the format uses, so a vault configuring both renders each format in
its own look.

Theme selection is a pure decision over those two sources; loading the
file is the one step that touches the filesystem, and it happens before
the vault is scanned so a bad name costs nothing. The loaded theme
travels into `Registry::new` beside the options, so both formats emit it
identically.

## Deferred

Not supported:

- rendering more than one note per invocation,
- built-in named themes beside the vault's own,
- a config surface for engine selection (per-format engine defaults,
  tool paths),
- a render action inside the search picker.
