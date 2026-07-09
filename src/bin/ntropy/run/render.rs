// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `render`: turn one note into a document artifact (ADR 0037, ADR 0038,
//! `docs/design/rendering.md`).
//!
//! The library owns the engine abstraction and performs no ambient effect
//! itself (ADR 0013). This module supplies the two things it deliberately does
//! not: the selector-to-single-note resolution shared with `delete`, and the
//! production [`RenderContext`] that stages files in a temporary directory and
//! spawns tools through `std::process::Command`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};

use ntropy::link;
use ntropy::ops;
use ntropy::render::{Invocation, Registry, RenderContext, RenderError, ToolOutput, prepare};
use ntropy::vault::Vault;

use crate::cli::GlobalArgs;

use super::{exit_for_warnings, output, picker, report_ambiguous};

/// Render the note named by `selector` to a document artifact.
///
/// The engine is resolved before the vault is even scanned, so an unknown
/// format or engine fails fast without any filesystem work. Selection then
/// mirrors `search`'s entry (a blank selector browses every note) combined
/// with `delete`'s exactly-one rule: several surviving notes open the picker
/// interactively and error under `-n`.
//
// Each parameter is an independent dispatch input threaded straight from the
// parsed CLI, exactly as the sibling `cmd_*` functions take theirs; bundling
// them into a struct would only rename the call site without removing an
// argument, so the lint is allowed rather than worked around.
#[allow(clippy::too_many_arguments)]
pub fn cmd_render(
    global: &GlobalArgs,
    vault: &Vault,
    selector: String,
    to: String,
    engine: Option<String>,
    output: Option<PathBuf>,
    print: bool,
    interactive: bool,
) -> Result<ExitCode> {
    // Resolve the engine first, before touching the vault: `--to`/`--engine`
    // are validated by the registry alone (the single authority), so a bad
    // value reports what exists without a wasted scan. The extension comes from
    // the same lookup, so the default output name is known up front.
    let registry = Registry::new();
    let renderer = registry
        .resolve(&to, engine.as_deref())
        .context("while selecting the render engine")?;
    let extension = registry
        .extension(&to)
        .context("while resolving the output extension")?
        .to_string();
    // The engine's name feeds the completion report; an explicit `--engine`
    // already carries it, otherwise it is the format's default.
    let engine_name = match &engine {
        Some(engine) => engine.clone(),
        None => registry
            .default_engine(&to)
            .context("while resolving the engine name")?
            .to_string(),
    };

    // A blank selector browses the whole vault, exactly as `search` enters its
    // picker; anything else resolves as a full ULID or a DSL query (the
    // id-or-query rule of ADR 0025). Scan warnings surface either way.
    let selector = super::optional(&selector).map(str::to_string);
    let matches = match selector.as_deref() {
        Some(selector) => {
            ops::resolve_selection(vault, selector).context("while resolving the selector")?
        }
        None => ops::search(vault, None).context("while listing notes")?,
    };
    output::print_warnings(&matches.warnings);

    // Narrow to exactly one note, honoring the ambiguity rule shared with
    // `delete` (ADR 0025). The picker yields a candidate, so the full note is
    // recovered from the resolved set by id, since rendering needs its body.
    let note = match matches.notes.as_slice() {
        [] => {
            match &selector {
                Some(selector) => eprintln!("error: no note matches `{selector}`"),
                None => eprintln!("No notes matched your search criteria."),
            }
            return Ok(ExitCode::FAILURE);
        }
        [note] => note.clone(),
        notes => {
            if interactive {
                let candidates = ops::to_candidates(notes)?;
                match picker::pick(candidates, picker::align_candidates)? {
                    Some(selected) => notes
                        .iter()
                        .find(|n| n.id == selected.id)
                        .expect("the picked candidate is one of the resolved notes")
                        .clone(),
                    // A cancelled picker fails under `-p` so
                    // `open "$(ntropy render -p ...)"` branches correctly;
                    // without it a cancel is a successful no-op like `delete`.
                    None if print => return Ok(ExitCode::FAILURE),
                    None => return Ok(ExitCode::SUCCESS),
                }
            } else {
                match &selector {
                    Some(selector) => report_ambiguous(selector, notes)?,
                    // Without a picker there is nothing to narrow a bare
                    // invocation with, so it asks for a selector instead of
                    // dumping every note as an ambiguity list.
                    None => eprintln!(
                        "error: rendering needs a selector in non-interactive mode ({} notes)",
                        notes.len()
                    ),
                }
                return Ok(ExitCode::FAILURE);
            }
        }
    };

    // The default artifact name is the note's slug plus the format's extension,
    // in the current directory; `-o` overrides it. An existing file is
    // overwritten (ADR 0037).
    let output_path = output.unwrap_or_else(|| default_output_path(&note.slug, &extension));

    // Announce the work before the engine runs: external typesetting can take
    // a few seconds, and under `--print` stdout stays reserved for the path
    // (ADR 0036), so the narration only appears without it.
    if !print {
        println!(
            "Rendering {}...",
            output::note_reference(&note).context("while formatting the note reference")?
        );
    }

    // Links resolve against the whole vault, so one extra scan builds the index
    // the preparation reads (resolved decision 5). Its warnings repeat the
    // resolution scan's on the same vault, so they are discarded here to avoid
    // printing each twice.
    let indexed = ops::search(vault, None).context("while indexing the vault for links")?;
    let index = link::index(&indexed.notes);
    let doc = prepare(&note, &index).context("while preparing the document")?;

    let mut ctx =
        ProcessContext::new(output_path.clone()).context("while creating the render workspace")?;
    renderer
        .render(&doc, &mut ctx)
        .context("while rendering the note")?;

    // `--print` writes exactly the artifact path to stdout for command
    // substitution (ADR 0036); otherwise a completion report names the
    // artifact, the format and engine that produced it, and its size.
    if print {
        println!("{}", output_path.display());
    } else {
        let size = std::fs::metadata(&output_path)
            .context("while reading the artifact's size")?
            .len();
        println!(
            "Rendered {} ({to} via {engine_name}, {})",
            output_path.display(),
            output::human_size(size)
        );
    }

    Ok(exit_for_warnings(global.strict, &matches.warnings))
}

/// The default artifact path: the note's slug joined to the format's extension,
/// relative to the current directory.
fn default_output_path(slug: &str, extension: &str) -> PathBuf {
    PathBuf::from(format!("{slug}.{extension}"))
}

/// The production [`RenderContext`]: a temporary staging directory plus real
/// subprocess execution.
///
/// The staging directory is owned by the [`tempfile::TempDir`], so it is
/// removed when the render ends however that happens. Tool probing lives next
/// to the spawn: a missing program surfaces as the library-defined
/// [`RenderError::RendererUnavailable`], naming what to install.
struct ProcessContext {
    staging: tempfile::TempDir,
    output: PathBuf,
}

impl ProcessContext {
    fn new(output: PathBuf) -> std::io::Result<Self> {
        let staging = tempfile::TempDir::new()?;
        Ok(Self { staging, output })
    }
}

impl RenderContext for ProcessContext {
    fn stage_file(&mut self, name: &str, contents: &[u8]) -> Result<PathBuf, RenderError> {
        let path = self.staging.path().join(name);
        std::fs::write(&path, contents).map_err(|source| RenderError::Stage {
            name: name.to_string(),
            source,
        })?;
        Ok(path)
    }

    fn run(&mut self, invocation: &Invocation) -> Result<ToolOutput, RenderError> {
        let output = std::process::Command::new(&invocation.program)
            .args(&invocation.args)
            .output();
        let output = match output {
            Ok(output) => output,
            // The tool is not on `PATH`: report it as unavailable, naming what
            // to install, rather than as a generic spawn failure.
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(RenderError::RendererUnavailable {
                    tool: invocation.program.clone(),
                    hint: "install pandoc and typst; both must be on PATH".to_string(),
                });
            }
            Err(source) => {
                return Err(RenderError::Spawn {
                    program: invocation.program.clone(),
                    source,
                });
            }
        };
        Ok(ToolOutput {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn output_path(&self) -> &Path {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_output_path_joins_slug_and_extension() {
        assert_eq!(
            default_output_path("quarterly-review", "pdf"),
            PathBuf::from("quarterly-review.pdf")
        );
    }

    #[test]
    fn stage_file_writes_inside_the_staging_dir() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let staged = ctx.stage_file("note.md", b"hello").expect("stage");
        assert!(staged.starts_with(ctx.staging.path()));
        assert_eq!(
            std::fs::read_to_string(&staged).expect("read staged"),
            "hello"
        );
    }

    #[test]
    fn staging_dir_is_removed_on_drop() {
        let ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let dir = ctx.staging.path().to_path_buf();
        assert!(dir.exists());
        drop(ctx);
        assert!(!dir.exists());
    }

    #[test]
    fn run_reports_a_missing_tool_as_unavailable() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "ntropy-test-definitely-missing-tool".to_string(),
            args: vec![],
        };
        let err = ctx.run(&invocation).expect_err("a missing tool errors");
        assert!(matches!(err, RenderError::RendererUnavailable { .. }));
    }

    #[test]
    fn run_captures_stdout_stderr_and_a_failure_status() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "echo out; echo err >&2; exit 3".into()],
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(!out.success);
        assert_eq!(out.stdout, b"out\n");
        assert_eq!(out.stderr, b"err\n");
    }

    #[test]
    fn run_reports_success_for_a_zero_exit() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "exit 0".into()],
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
    }
}
