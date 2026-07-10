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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

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
    // The vault's render options shape the engines, so the config loads before
    // the registry is built; a broken config (or an unknown paper name) fails
    // here, before any scan.
    let config = ntropy::config::PerVaultConfig::load(&vault.layout().config_file())
        .context("while loading the vault config")?;

    // Resolve the engine next, still before touching the vault's notes:
    // `--to`/`--engine` are validated by the registry alone (the single
    // authority), so a bad value reports what exists without a wasted scan. The
    // extension comes from the same lookup, so the default output name is known
    // up front.
    let registry = Registry::new(config.render);
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

    // The engine may run its tool in the note's own directory (the typst pdf
    // pipeline does, so relative assets resolve against the note). A relative
    // output path would then resolve inside that directory rather than where the
    // user invoked the command, so it is absolutized against the process's
    // working directory before the engine ever sees it. The user-facing prints
    // below keep the original, as-given form.
    let cwd = std::env::current_dir().context("while resolving the current directory")?;
    let absolute_output = absolutize(&output_path, &cwd);

    let mut ctx =
        ProcessContext::new(absolute_output).context("while creating the render workspace")?;
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

    // Engine degradation warnings (dropped HTML, remote images) fail a strict
    // run just like scan warnings, so they fold into the same exit decision.
    Ok(exit_for_warnings(
        global.strict,
        &matches.warnings,
        ctx.warning_count(),
    ))
}

/// The default artifact path: the note's slug joined to the format's extension,
/// relative to the current directory.
fn default_output_path(slug: &str, extension: &str) -> PathBuf {
    PathBuf::from(format!("{slug}.{extension}"))
}

/// Resolve `path` to an absolute location, joining a relative path onto `cwd`
/// and leaving an already-absolute path untouched.
///
/// The engine receives the absolute form so its tool's working directory cannot
/// change where the artifact lands; the user still sees the path as given.
fn absolutize(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// Resolve `program` to a concrete, executable path in the caller's context.
///
/// A bare program name is searched along `path_var` in order; each relative
/// `PATH` entry is joined against `parent_cwd` rather than left for the OS to
/// resolve. This matters because a child spawned with its own working directory
/// resolves relative `PATH` entries against *that* directory under Unix
/// `execvp`, not against the caller's — so a relative `PATH` entry would mean
/// two different things depending on where the child runs. Resolving here, in
/// the caller's directory, pins the entry to the location the caller intends.
/// A program that already contains a path separator is taken as a path and
/// absolutized against `parent_cwd`.
///
/// Returns the first candidate that exists and is executable, or `None` when no
/// such file is found (the tool is absent).
fn resolve_program(program: &str, path_var: &str, parent_cwd: &Path) -> Option<PathBuf> {
    // A path-bearing name is a location, not a `PATH` lookup. Absolutizing it
    // against the caller's directory makes it independent of the child's cwd.
    if program.contains(std::path::MAIN_SEPARATOR) {
        let candidate = parent_cwd.join(program);
        return is_executable_file(&candidate).then_some(candidate);
    }

    path_var.split(':').find_map(|entry| {
        // An empty entry (a leading, trailing, or doubled `:`) conventionally
        // means the current directory; every other relative entry anchors to
        // the caller's cwd, absolute entries stand on their own.
        let dir = if entry.is_empty() {
            parent_cwd.to_path_buf()
        } else {
            parent_cwd.join(entry)
        };
        let candidate = dir.join(program);
        is_executable_file(&candidate).then_some(candidate)
    })
}

/// Whether `path` is a regular file with at least one execute bit set.
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
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
    /// The number of degradation warnings the engine reported, so the caller
    /// can fold them into the `--strict` exit decision alongside scan warnings.
    warnings: usize,
}

impl ProcessContext {
    fn new(output: PathBuf) -> std::io::Result<Self> {
        let staging = tempfile::TempDir::new()?;
        Ok(Self {
            staging,
            output,
            warnings: 0,
        })
    }

    /// How many degradation warnings the engine reported during the render.
    fn warning_count(&self) -> usize {
        self.warnings
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
        // Resolve the program to a concrete path here, in the parent's context,
        // rather than letting the spawn resolve a bare name against `PATH`. The
        // child may run in a different working directory (see `cwd` below), and
        // Unix `execvp` would resolve relative `PATH` entries against *that*
        // directory; resolving up front keeps a relative `PATH` entry meaning
        // what the invoking process intends. A resolution miss is the tool being
        // absent, so it surfaces as unavailable, naming what to install.
        let path_var = std::env::var("PATH").unwrap_or_default();
        let parent_cwd = std::env::current_dir().map_err(|source| RenderError::Spawn {
            program: invocation.program.clone(),
            source,
        })?;
        let program =
            resolve_program(&invocation.program, &path_var, &parent_cwd).ok_or_else(|| {
                RenderError::RendererUnavailable {
                    tool: invocation.program.clone(),
                    hint: format!("install {}; it must be on PATH", invocation.program),
                }
            })?;

        let mut command = Command::new(&program);
        command.args(&invocation.args);
        if let Some(cwd) = &invocation.cwd {
            command.current_dir(cwd);
        }

        // Map a spawn failure the same way regardless of which path builds the
        // command: a `NotFound` can still occur if the resolved file is deleted
        // between resolution and spawn, so it remains an unavailability report.
        let spawn_error = |source: std::io::Error| -> RenderError {
            if source.kind() == std::io::ErrorKind::NotFound {
                RenderError::RendererUnavailable {
                    tool: invocation.program.clone(),
                    hint: format!("install {}; it must be on PATH", invocation.program),
                }
            } else {
                RenderError::Spawn {
                    program: invocation.program.clone(),
                    source,
                }
            }
        };

        let output = match &invocation.stdin {
            // With a payload the child reads from a pipe. The write happens on a
            // separate thread that drops the handle when done, signalling EOF,
            // while this thread drains stdout/stderr through `wait_with_output`:
            // decoupling the two directions is what keeps a large payload from
            // deadlocking against a child that fills its stdout pipe mid-read.
            Some(payload) => {
                let mut child = command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(spawn_error)?;

                let mut stdin = child
                    .stdin
                    .take()
                    .expect("stdin was configured as a pipe just above");
                let payload = payload.clone();
                let writer = std::thread::spawn(move || stdin.write_all(&payload));

                let output = child
                    .wait_with_output()
                    .map_err(|source| RenderError::Spawn {
                        program: invocation.program.clone(),
                        source,
                    })?;
                // The child may legitimately close stdin early (typst reads only
                // what it needs), so a broken-pipe write is expected, not fatal;
                // the tool's own exit status is the authority on success.
                let _ = writer.join().expect("stdin writer thread does not panic");
                output
            }
            // No payload: `Command::output` gives the child a null stdin, so any
            // read closes immediately, and captures stdout/stderr as before.
            None => command.output().map_err(spawn_error)?,
        };

        Ok(ToolOutput {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn write_output(&mut self, contents: &[u8]) -> Result<(), RenderError> {
        std::fs::write(&self.output, contents).map_err(|source| RenderError::WriteOutput { source })
    }

    fn warn(&mut self, message: &str) {
        // Engine warnings go to stderr, keeping the `-p` stdout contract clean
        // (ADR 0036), in the same visual style as scan warnings.
        eprintln!("warning: {message}");
        self.warnings += 1;
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
    fn absolutize_joins_a_relative_path_onto_the_cwd() {
        assert_eq!(
            absolutize(Path::new("sub/out.pdf"), Path::new("/work/vault")),
            PathBuf::from("/work/vault/sub/out.pdf")
        );
    }

    #[test]
    fn absolutize_leaves_an_absolute_path_unchanged() {
        assert_eq!(
            absolutize(Path::new("/tmp/out.pdf"), Path::new("/work/vault")),
            PathBuf::from("/tmp/out.pdf")
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
            stdin: None,
            cwd: None,
        };
        let err = ctx.run(&invocation).expect_err("a missing tool errors");
        match err {
            RenderError::RendererUnavailable { tool, hint } => {
                assert_eq!(tool, "ntropy-test-definitely-missing-tool");
                assert_eq!(
                    hint,
                    "install ntropy-test-definitely-missing-tool; it must be on PATH"
                );
            }
            other => panic!("expected RendererUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn run_reports_a_nonexistent_absolute_path_as_unavailable() {
        // An absolute program path that does not exist is the tool being absent,
        // not a `PATH` miss, so it too surfaces as unavailable with the hint.
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "/no/such/dir/ntropy-missing".to_string(),
            args: vec![],
            stdin: None,
            cwd: None,
        };
        let err = ctx
            .run(&invocation)
            .expect_err("a missing absolute path errors");
        assert!(matches!(err, RenderError::RendererUnavailable { .. }));
    }

    #[test]
    fn run_captures_stdout_stderr_and_a_failure_status() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "echo out; echo err >&2; exit 3".into()],
            stdin: None,
            cwd: None,
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(!out.success);
        assert_eq!(out.stdout, b"out\n");
        assert_eq!(out.stderr, b"err\n");
    }

    #[test]
    fn write_output_writes_bytes_to_the_output_path() {
        let staging = tempfile::TempDir::new().expect("temp dir");
        let out = staging.path().join("note.typ");
        let mut ctx = ProcessContext::new(out.clone()).expect("context");
        ctx.write_output(b"#show: note.with(title: \"T\",)")
            .expect("write succeeds");
        assert_eq!(
            std::fs::read_to_string(&out).expect("read artifact"),
            "#show: note.with(title: \"T\",)"
        );
    }

    #[test]
    fn write_output_maps_an_unwritable_path_to_write_output_error() {
        // A path whose parent directory does not exist cannot be written, so the
        // io error maps to the dedicated variant rather than escaping raw.
        let mut ctx = ProcessContext::new(PathBuf::from("/no/such/dir/note.typ")).expect("context");
        let err = ctx
            .write_output(b"body")
            .expect_err("an unwritable path errors");
        assert!(matches!(err, RenderError::WriteOutput { .. }));
    }

    #[test]
    fn warn_counts_reported_warnings() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.typ")).expect("context");
        assert_eq!(ctx.warning_count(), 0);
        ctx.warn("remote image dropped");
        ctx.warn("raw HTML dropped");
        assert_eq!(ctx.warning_count(), 2);
    }

    #[test]
    fn run_reports_success_for_a_zero_exit() {
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "exit 0".into()],
            stdin: None,
            cwd: None,
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
    }

    #[test]
    fn run_honors_the_invocation_cwd() {
        // `pwd` prints the child's working directory; setting `cwd` must move it
        // there rather than leaving it in the host's current directory.
        let dir = tempfile::TempDir::new().expect("temp dir");
        // Canonicalize so the assertion is immune to symlinked temp roots (macOS
        // `/tmp` → `/private/tmp`), which `pwd -P` also resolves.
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "pwd -P".into()],
            stdin: None,
            cwd: Some(canonical.clone()),
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim_end(),
            canonical.to_string_lossy()
        );
    }

    #[test]
    fn run_delivers_stdin_to_the_child() {
        // `cat` echoes stdin to stdout, so the captured stdout must equal the
        // payload byte-for-byte, proving the pipe is wired and closed.
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let payload = b"the emitted typst document".to_vec();
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "cat".into()],
            stdin: Some(payload.clone()),
            cwd: None,
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
        assert_eq!(out.stdout, payload);
    }

    #[test]
    fn run_streams_a_large_payload_without_deadlock() {
        // A payload well past a pipe buffer must flow through `cat` intact: if
        // the writer and the stdout drain shared one thread, this would hang.
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let payload = vec![b'x'; 4 * 1024 * 1024];
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "cat".into()],
            stdin: Some(payload.clone()),
            cwd: None,
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
        assert_eq!(out.stdout.len(), payload.len());
        assert_eq!(out.stdout, payload);
    }

    #[test]
    fn run_handles_a_child_writing_heavily_while_reading_stdin() {
        // The child reads a large stdin while emitting a large stdout: both pipes
        // fill at once, so only concurrent read and write keeps it from wedging.
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let payload = vec![b'y'; 2 * 1024 * 1024];
        // `tee` copies stdin to stdout while also draining it, and `yes` floods
        // stdout independently; a `head` bounds the flood to a fixed large size.
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "cat; yes ntropy | head -c 3000000".into()],
            stdin: Some(payload.clone()),
            cwd: None,
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
        // stdout is the echoed payload followed by the 3 MiB flood.
        assert_eq!(out.stdout.len(), payload.len() + 3_000_000);
    }

    #[test]
    fn run_without_stdin_still_captures_output() {
        // The no-payload path keeps `Command::output` semantics: the child gets a
        // null stdin and its stdout is captured all the same.
        let mut ctx = ProcessContext::new(PathBuf::from("out.pdf")).expect("context");
        let invocation = Invocation {
            program: "sh".to_string(),
            args: vec!["-c".into(), "echo produced".into()],
            stdin: None,
            cwd: None,
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
        assert_eq!(out.stdout, b"produced\n");
    }

    // =========================================================
    // Parent-context program resolution
    // =========================================================

    /// A `mode 0o755` file at `path`, so resolution finds an executable
    /// candidate without invoking it.
    fn write_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, "#!/bin/sh\nexit 0\n").expect("write fake executable");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake executable");
    }

    #[test]
    fn resolve_program_finds_a_bare_name_on_an_absolute_path_entry() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let bin = dir.path().join("mytool");
        write_executable(&bin);
        let resolved = resolve_program(
            "mytool",
            &dir.path().to_string_lossy(),
            Path::new("/unused/parent"),
        )
        .expect("the tool resolves");
        assert_eq!(resolved, bin);
    }

    #[test]
    fn resolve_program_joins_a_relative_path_entry_against_the_parent_cwd() {
        // The load-bearing case: a relative `PATH` entry resolves against the
        // caller's cwd, so a child running elsewhere still finds the tool where
        // the caller placed it. The invocation's own `cwd` is deliberately
        // different to prove resolution ignores it.
        let parent = tempfile::TempDir::new().expect("parent dir");
        let bin_dir = parent.path().join("stub-bin");
        std::fs::create_dir_all(&bin_dir).expect("stub-bin dir");
        write_executable(&bin_dir.join("mytool"));

        let resolved = resolve_program("mytool", "stub-bin", parent.path())
            .expect("the relative entry resolves against the parent cwd");
        assert_eq!(resolved, bin_dir.join("mytool"));
    }

    #[test]
    fn resolve_program_searches_entries_in_order() {
        // Two entries both hold the tool; the first on `PATH` wins.
        let first = tempfile::TempDir::new().expect("first dir");
        let second = tempfile::TempDir::new().expect("second dir");
        write_executable(&first.path().join("mytool"));
        write_executable(&second.path().join("mytool"));
        let path_var = format!(
            "{}:{}",
            first.path().to_string_lossy(),
            second.path().to_string_lossy()
        );
        let resolved =
            resolve_program("mytool", &path_var, Path::new("/unused")).expect("the tool resolves");
        assert_eq!(resolved, first.path().join("mytool"));
    }

    #[test]
    fn resolve_program_skips_a_non_executable_candidate() {
        // A same-named but non-executable file in an earlier entry is skipped in
        // favor of the executable one later on `PATH`.
        let first = tempfile::TempDir::new().expect("first dir");
        let second = tempfile::TempDir::new().expect("second dir");
        std::fs::write(first.path().join("mytool"), "not executable").expect("write plain file");
        write_executable(&second.path().join("mytool"));
        let path_var = format!(
            "{}:{}",
            first.path().to_string_lossy(),
            second.path().to_string_lossy()
        );
        let resolved = resolve_program("mytool", &path_var, Path::new("/unused"))
            .expect("the executable candidate resolves");
        assert_eq!(resolved, second.path().join("mytool"));
    }

    #[test]
    fn resolve_program_returns_none_for_an_absent_bare_name() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        assert!(
            resolve_program(
                "mytool",
                &dir.path().to_string_lossy(),
                Path::new("/unused")
            )
            .is_none()
        );
    }

    #[test]
    fn resolve_program_takes_a_path_bearing_name_as_a_location() {
        // A name containing a separator is a path, absolutized against the parent
        // cwd, not a `PATH` lookup.
        let parent = tempfile::TempDir::new().expect("parent dir");
        let sub = parent.path().join("sub");
        std::fs::create_dir_all(&sub).expect("sub dir");
        write_executable(&sub.join("mytool"));

        let resolved = resolve_program("sub/mytool", "/ignored", parent.path())
            .expect("the path-bearing name resolves against the parent cwd");
        assert_eq!(resolved, parent.path().join("sub/mytool"));
    }

    #[test]
    fn resolve_program_rejects_a_path_bearing_name_that_is_not_executable() {
        let parent = tempfile::TempDir::new().expect("parent dir");
        std::fs::write(parent.path().join("plain"), "not executable").expect("write plain file");
        assert!(resolve_program("./plain", "/ignored", parent.path()).is_none());
    }
}
