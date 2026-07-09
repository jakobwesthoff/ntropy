# Rendering

The `render` command and its engine: how a note becomes an output artifact
such as a PDF. Decisions are recorded in
[ADR 0037](../adr/0037-render-command-surface.md) and
[ADR 0038](../adr/0038-pluggable-rendering-engine-with-pandoc-and-typst.md);
selector semantics in [query-and-search.md](query-and-search.md), the overall
command surface in [cli.md](cli.md).

v1 renders exactly one note per invocation and ships one format, `pdf`,
produced by one engine, `pandoc`, which delegates typesetting to typst.

## CLI surface

    ntropy render <id|query> [--to <format>] [--engine <name>] [-o <path>] [-p]

- The selector follows the id-or-query rule shared with `search` and
  `delete`: a full 26-char ULID resolves directly to that note, anything
  else runs as a DSL query. It is required; `render` has no
  browse-everything mode.
- Like `delete`, `render` must resolve to exactly one note: an ambiguous
  selector opens the picker pre-filtered interactively, and errors with the
  candidate list under `-n` (ADR 0025). Interactivity keys off the
  controlling terminal (ADR 0036). A cancelled picker exits non-zero under
  `-p`, so `open "$(ntropy render -p ...)"` branches correctly, and is a
  successful no-op without it, like `delete`.
- `--to <format>` selects the output format and defaults to `pdf`.
- `--engine <name>` overrides the format's default engine. v1 has one
  engine, `pandoc`, so the flag accepts only that value; it exists so that
  invocations written today keep working when alternative engines arrive.
- `--output <path>` / `-o` names the artifact. The default is
  `./<slug>.pdf` in the current working directory, where `<slug>` is the
  slug component of the note's filename. An existing file at the target is
  overwritten.
- `--print` / `-p` prints the artifact's path to stdout as one line on
  success, so `open "$(ntropy render -p ...)"` composes. Without it,
  nothing is written to stdout; the artifact file is the outcome.
- Scan warnings print to stderr and fail the command under `--strict`,
  matching `search`.
- `render` is read-only with respect to the vault: nothing was edited, so
  there is no filename realignment and no view refresh.

## Formats and engines

A **format** is the artifact kind the user asks for (`pdf`). An **engine**
is an implementation able to produce one or more formats. The registry maps
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
/// Everything an engine may ask the host to do.
pub trait RenderContext {
    /// Materialize an intermediate file in a render-scoped workspace.
    fn stage_file(&mut self, name: &str, contents: &[u8]) -> Result<PathBuf, RenderError>;
    /// Execute an external tool and return its captured output.
    fn run(&mut self, invocation: &Invocation) -> Result<ToolOutput, RenderError>;
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

An engine's chain is ordinary sequential Rust inside `render`: stage files,
run tools, derive later steps from earlier output. Multi-step chains,
conditionals, and intermediate artifacts need no plan language, because the
logic never leaves the library; only the effects do.

The binary supplies the single production `RenderContext`: `stage_file`
backed by a temporary directory that is removed when the render ends, and
`run` backed by `std::process::Command`. That one implementation serves
every engine, so adding an engine never touches the binary. Probing for an
engine's external tools is likewise the binary's job, next to the spawn;
the library defines the error the probe reports through.

This is a capability reading of ADR 0013's headless rule: the library still
contains no spawn call and no ambient effect; it requests effects through a
context handed in by its host. Tests hand in a fake context instead (see
Testing).

The context grows a primitive only when a real engine needs it. v1 needs
exactly the three methods above.

## The pandoc engine

The v1 `pdf` engine converts the note with pandoc and delegates PDF
typesetting to typst: pandoc's typst PDF engine runs the `typst` binary
itself, so both tools must be installed. Both are found via `PATH` (pandoc
by ntropy, typst by pandoc); there are no configurable tool paths in v1. A
missing tool fails the render with an error naming what to install.

Materialization, the lossy half owned by this engine:

- The body is staged as a Markdown file with the frontmatter stripped.
  Each resolved link is replaced by the target note's current title as
  emphasized text (`*Title*`); an unresolved link keeps its display text.
- Metadata travels as `--metadata` arguments: `title` (the note title),
  `date` (the prepared creation date), `subtitle` (the tags, each
  `#`-prefixed, joined with ` · `), and `keywords` (the tags,
  comma-joined). Title, date, and tags are typeset by pandoc's stock typst
  template, and the tags also land in the PDF's document metadata.

The invocation:

    pandoc <staged.md> --from gfm --pdf-engine=typst \
        --metadata title=... --metadata date=... \
        --metadata subtitle=... --metadata keywords=... \
        --output <artifact.pdf>

`--from gfm` pins the reading of note bodies to GitHub-flavored Markdown
rather than pandoc's own dialect, whose extensions (such as citation
syntax) would give plain note text special meaning.

Appearance is pandoc's stock typst output; v1 has no template or styling
configuration.

## Testing

The engine seam is built for ADR 0021's snapshot style without the external
tools installed:

- Preparation: the `PreparedDocument` built from fixture notes is
  snapshot with `insta`.
- Engines: tests hand `render` a fake `RenderContext` that records every
  `stage_file` and `run` call and feeds back scripted outputs. The recorded
  sequence, staged contents and full argv included, is the snapshot,
  pinning the exact pandoc invocation without executing pandoc.
- CLI contract tests exercise the command end-to-end through a test-owned
  stub `pandoc` placed on `PATH`; the real pandoc and typst are never
  executed by tests, so the real-toolchain output stays validated manually.
  Contract tests pass `-n` or `--print` per ADR 0036.

## Module layout

- `src/render/` (library): the document model and shared preparation, the
  format/engine registry, `Renderer`, `RenderContext`, `RenderError`, and
  the pandoc engine's chain logic.
- `src/bin/ntropy/run/render.rs` (binary): `cmd_render` (selector
  resolution, picker on ambiguity, output-path defaulting) and the
  production `RenderContext`.

The selector plumbing is reused as-is: `ops::resolve_selection` and the
generic picker already serve `search` and `delete` unchanged.

## Deferred beyond v1

Not part of v1:

- rendering more than one note per invocation,
- styling and template control,
- a config surface for rendering (per-format engine defaults, tool paths),
- a render action inside the search picker,
- further formats and engines.
