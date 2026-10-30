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

use std::borrow::Cow;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, Result, bail};

use ntropy::link;
use ntropy::ops;
use ntropy::render::{Invocation, Registry, RenderContext, RenderError, ToolOutput, prepare};
use ntropy::session::VaultSession;
use ntropy::text::escape_controls;

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
    session: &VaultSession,
    selector: String,
    to: String,
    engine: Option<String>,
    output: Option<PathBuf>,
    theme: Option<String>,
    force: bool,
    print: bool,
    interactive: bool,
) -> Result<ExitCode> {
    // The vault's render options shape the engines, so the config loads before
    // the registry is built; a broken config (or an unknown paper name) fails
    // here, before any scan.
    let config = ntropy::config::PerVaultConfig::load(&session.layout().config_file())
        .context("while loading the vault config")?;

    // Resolve the engine next, still before touching the vault's notes:
    // `--to`/`--engine` are validated by the registry alone (the single
    // authority), so a bad value reports what exists without a wasted scan. The
    // extension comes from the same lookup, so the default output name is known
    // up front.
    // The theme is selected from `--theme` over the vault's configured one and
    // loaded before anything else, so a name that resolves to no file fails
    // here rather than after a scan and a compile (ADR 0045). `default` from
    // either source means the built-in look and reads no file. Which theme
    // kind `--theme` names follows the format: the `html` format takes a site
    // theme from `[site] theme` and `themes/site/`, every other format a Typst
    // theme from `[render] theme` and `themes/typst/` (ADR 0048).
    let mut loaded_theme = None;
    let mut site = ntropy::site::DocumentSettings {
        theme: None,
        theme_dir: None,
        lang: config.site.lang().to_string(),
        vars: config.site.vars.clone(),
    };
    if to == "html" {
        let selected =
            ntropy::render::theme::select(theme.as_deref(), config.site.theme.as_deref());
        site.theme = selected
            .map(|name| ntropy::site::theme::load(session.layout(), name))
            .transpose()
            .context("while loading the site theme")?;
        site.theme_dir = selected.map(|name| session.layout().site_theme_dir(name));
    } else {
        let selected =
            ntropy::render::theme::select(theme.as_deref(), config.render.theme.as_deref());
        loaded_theme = selected
            .map(|name| ntropy::render::theme::load(session.layout(), name))
            .transpose()
            .context("while loading the render theme")?;
    }

    let registry = Registry::new(config.render.clone(), loaded_theme, site);
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
            ops::resolve_selection(session, selector).context("while resolving the selector")?
        }
        None => ops::search(session, None).context("while listing notes")?,
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
    // in the current directory; `-o` overrides it. A path `--print` could not
    // print as it is fails before the engine writes anything there (ADR 0071).
    let output_path = match output {
        Some(path) => {
            if print {
                output::checked_path(&path, false)?;
            }
            path
        }
        None => {
            let path = default_output_path(&note.slug, &extension);
            check_default_name(&path)?;
            path
        }
    };

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
    let indexed = ops::search(session, None).context("while indexing the vault for links")?;
    let index = link::index(&indexed.notes);
    let doc = prepare(&note, &index, session.root()).context("while preparing the document")?;

    // A relative output path is joined onto the process's working directory
    // before the engine sees it. The vault-containment check below compares it
    // with the vault root, and `typst` would read a relative name starting with
    // `-` as an option and a lone `-` as standard output. The user-facing prints
    // below keep the as-given form.
    let cwd = std::env::current_dir().context("while resolving the current directory")?;
    let absolute_output = absolutize(&output_path, &cwd);

    // An existing artifact, and for `html` a non-empty files directory
    // beside it, are render targets of an earlier run: neither is replaced
    // without `--force` (ADR 0057), which then clears the directory so no
    // stale file of an earlier render lingers in it.
    let files = ntropy::render::files_dir(&absolute_output);
    let files_shown = files
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !force {
        if absolute_output.exists() {
            bail!(
                "`{}` exists; pass --force to replace it",
                output_path.display()
            );
        }
        if to == "html" && dir_has_entries(&files) {
            bail!("`{files_shown}/` is not empty; pass --force to replace it");
        }
    } else if to == "html" && files.is_dir() {
        std::fs::remove_dir_all(&files)
            .with_context(|| format!("while clearing `{files_shown}/`"))?;
    }

    // A rendered artifact is plaintext by nature. The common accident is a
    // shell whose working directory happens to be the vault, where the default
    // `./<name>.pdf` would drop a readable copy of the note straight into the
    // synced directory.
    if lands_inside_vault(session.root(), &absolute_output) && session.is_encrypted() {
        eprintln!(
            "warning: `{}` is inside an encrypted vault; the artifact is not \
             encrypted and will sync as plaintext",
            output_path.display()
        );
    }

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
            output::shown_path(&output_path),
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

/// Refuse a default artifact name that a canonical slug could not produce
/// (ADR 0071).
///
/// A plaintext note's slug is the rest of its file name as it is on disk, so
/// a file ntropy did not create can carry characters that would reach the
/// terminal, or a leading `-` that makes the name read as an option. The
/// refusal comes before anything is written: `ntropy reconcile` gives the
/// note its canonical name, or `-o` names the artifact. The name is not
/// taken from the title instead, as other artifacts link to this one by it
/// (ADR 0044).
fn check_default_name(path: &Path) -> Result<()> {
    const REMEDY: &str =
        "run `ntropy reconcile` to rename the note, or name the artifact with `-o`";
    let name = path.to_string_lossy();
    if let Cow::Owned(escaped) = escape_controls(&name) {
        bail!("the default artifact name `{escaped}` holds control or bidi characters; {REMEDY}");
    }
    if name.starts_with('-') {
        bail!("the default artifact name `{name}` starts with `-`; {REMEDY}");
    }
    Ok(())
}

/// Whether `dir` is a directory holding at least one entry.
fn dir_has_entries(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Ignore `SIGPIPE` process-wide for the lifetime of the value, restoring the
/// previous disposition on drop.
///
/// A pipe write to a closed read end raises `SIGPIPE`; with the signal
/// ignored, the write fails with `EPIPE` instead, which the caller handles as
/// an ordinary error. The disposition is process-wide state (a thread-local
/// mask does not contain it on macOS, where the signal is delivered
/// process-wide), so the scope is kept to the piped tool exchange and the
/// previous disposition — the fatal default that gives ntropy its quiet exit
/// when its own stdout pipe closes — comes back the moment the scope ends.
struct SigpipeIgnoredScope {
    previous: libc::sighandler_t,
}

impl SigpipeIgnoredScope {
    fn new() -> Self {
        // SAFETY: `SIGPIPE` and `SIG_IGN` are valid signal/handler constants,
        // and `signal` returns the previous handler for the restore.
        let previous = unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
        SigpipeIgnoredScope { previous }
    }
}

impl Drop for SigpipeIgnoredScope {
    fn drop(&mut self) {
        // SAFETY: `self.previous` came from the paired `signal` call above.
        unsafe {
            libc::signal(libc::SIGPIPE, self.previous);
        }
    }
}

/// Whether `output` would be written inside `vault_root`.
///
/// Component-wise, which `Path::starts_with` already is: a sibling directory
/// whose name merely begins with the vault's — `/v/vault-notes` beside
/// `/v/vault` — is not inside it.
pub(super) fn lands_inside_vault(vault_root: &Path, output: &Path) -> bool {
    output.starts_with(vault_root)
}

/// Resolve `path` to an absolute location, joining a relative path onto `cwd`
/// and leaving an already-absolute path untouched.
///
/// `render` and `site` compare the result with the vault root, and `render`
/// hands it to the engine's tool, where an absolute path cannot be read as an
/// option or as `-` for standard output. The user still sees the path as given.
pub(super) fn absolutize(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// The error for a tool `program` that failed to start with `source`.
///
/// An absent or unusable tool is reported as unavailable, naming what to
/// install. A `NotFound` spawn means the program, or the interpreter a script
/// names, is missing. A `PATH` search that met only an unsearchable directory,
/// a non-executable file or a directory of that name ends in
/// `PermissionDenied`, and on Linux one whose last entry is a file ends in
/// `NotADirectory`. Any other spawn failure keeps its own cause.
fn spawn_failure(program: &str, source: std::io::Error) -> RenderError {
    use std::io::ErrorKind;

    match source.kind() {
        ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::NotADirectory => {
            RenderError::RendererUnavailable {
                tool: program.to_string(),
                hint: format!("install {program}; it must be on PATH"),
            }
        }
        _ => RenderError::Spawn {
            program: program.to_string(),
            source,
        },
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
    /// The artifact's files directory, `<stem>_files/` beside it.
    files: PathBuf,
    /// The number of degradation warnings the engine reported, so the caller
    /// can fold them into the `--strict` exit decision alongside scan warnings.
    warnings: usize,
}

impl ProcessContext {
    fn new(output: PathBuf) -> std::io::Result<Self> {
        // Staged in the runtime directory rather than wherever `TempDir`
        // defaults to, so an encrypted note's body never lands somewhere the
        // rest of the system can read while an engine works on it (ADR 0041).
        let staging = tempfile::TempDir::new_in(super::securetemp::runtime_dir())?;
        Ok(Self {
            staging,
            files: ntropy::render::files_dir(&output),
            output,
            warnings: 0,
        })
    }

    /// The path of `relative` inside the files directory, its parent
    /// directories created. A `relative` that leaves the files directory is
    /// refused (ADR 0072).
    fn file_path(&self, relative: &str) -> Result<PathBuf, RenderError> {
        let path = ntropy::render::join_within(&self.files, relative).ok_or_else(|| {
            RenderError::OutsideFilesDir {
                name: relative.to_string(),
            }
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| RenderError::WriteFile {
                name: relative.to_string(),
                source,
            })?;
        }
        Ok(path)
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
        // The tool runs in ntropy's own working directory, and the spawn looks a
        // bare program name up on `PATH` from there.
        let mut command = Command::new(&invocation.program);
        command.args(&invocation.args);
        let spawn_error = |source| spawn_failure(&invocation.program, source);

        let output = match &invocation.stdin {
            // With a payload the child reads from a pipe. The write happens on a
            // separate thread that drops the handle when done, signalling EOF,
            // while this thread drains stdout/stderr through `wait_with_output`:
            // decoupling the two directions is what keeps a large payload from
            // deadlocking against a child that fills its stdout pipe mid-read.
            Some(payload) => {
                // The process runs with `SIGPIPE` at its default, fatal
                // disposition (see `main`), and a pipe write to a child that
                // already exited raises it — on macOS delivered process-wide,
                // so a thread-local signal mask cannot contain it. Ignoring
                // the signal for the duration of the piped exchange turns
                // that write into a plain `EPIPE` error; the guard restores
                // the previous disposition on every exit path, keeping the
                // quiet-exit behavior for ntropy's own stdout intact.
                let _sigpipe = SigpipeIgnoredScope::new();

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

    fn write_file(&mut self, relative: &str, contents: &[u8]) -> Result<(), RenderError> {
        let path = self.file_path(relative)?;
        std::fs::write(path, contents).map_err(|source| RenderError::WriteFile {
            name: relative.to_string(),
            source,
        })
    }

    fn copy_file(&mut self, from: &Path, relative: &str) -> Result<(), RenderError> {
        let path = self.file_path(relative)?;
        std::fs::copy(from, path)
            .map(|_| ())
            .map_err(|source| RenderError::WriteFile {
                name: relative.to_string(),
                source,
            })
    }

    fn warn(&mut self, message: &str) {
        // Engine warnings go to stderr, keeping the `-p` stdout contract clean
        // (ADR 0036), in the same visual style as scan warnings.
        output::warn(message);
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
    fn an_output_inside_the_vault_is_detected() {
        assert!(lands_inside_vault(
            Path::new("/v/vault"),
            Path::new("/v/vault/report.pdf")
        ));
        assert!(lands_inside_vault(
            Path::new("/v/vault"),
            Path::new("/v/vault/sub/report.pdf")
        ));
    }

    #[test]
    fn an_output_outside_the_vault_is_not() {
        assert!(!lands_inside_vault(
            Path::new("/v/vault"),
            Path::new("/tmp/report.pdf")
        ));
    }

    #[test]
    fn a_sibling_sharing_a_name_prefix_is_not_inside() {
        // `Path::starts_with` compares components, not characters. A string
        // comparison would call this a hit and warn about a directory that has
        // nothing to do with the vault.
        assert!(!lands_inside_vault(
            Path::new("/v/vault"),
            Path::new("/v/vault-notes/report.pdf")
        ));
    }

    #[test]
    fn the_vault_root_itself_counts_as_inside() {
        assert!(lands_inside_vault(
            Path::new("/v/vault"),
            Path::new("/v/vault")
        ));
    }

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
    fn write_file_writes_inside_the_files_directory() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let mut ctx = ProcessContext::new(tmp.path().join("out.html")).expect("context");
        ctx.write_file("grammars/x.js", b"grammar")
            .expect("a path below the files directory is written");
        assert_eq!(
            std::fs::read(tmp.path().join("out_files/grammars/x.js")).expect("written"),
            b"grammar"
        );
    }

    #[test]
    fn write_file_refuses_a_path_leaving_the_files_directory() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let mut ctx = ProcessContext::new(tmp.path().join("out.html")).expect("context");
        let err = ctx
            .write_file("../escape.txt", b"escaped")
            .expect_err("a path above the files directory is refused");
        assert!(
            matches!(&err, RenderError::OutsideFilesDir { name } if name == "../escape.txt"),
            "{err:?}"
        );
        assert!(!tmp.path().join("escape.txt").exists());
    }

    #[test]
    fn copy_file_refuses_an_absolute_path() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let source = tmp.path().join("source.txt");
        std::fs::write(&source, b"source").expect("source");
        let elsewhere = tmp.path().join("elsewhere/x");
        let mut ctx = ProcessContext::new(tmp.path().join("out.html")).expect("context");
        let err = ctx
            .copy_file(&source, &elsewhere.to_string_lossy())
            .expect_err("an absolute target is refused");
        assert!(
            matches!(err, RenderError::OutsideFilesDir { .. }),
            "{err:?}"
        );
        assert!(!elsewhere.exists());
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
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
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
        };
        let out = ctx.run(&invocation).expect("sh runs");
        assert!(out.success);
        assert_eq!(out.stdout, b"produced\n");
    }

    // =========================================================
    // Spawn failure mapping
    // =========================================================

    /// Assert that a spawn failing with `kind` reports `typst` as unavailable,
    /// with the install hint.
    fn assert_unavailable_for(kind: std::io::ErrorKind) {
        match spawn_failure("typst", std::io::Error::from(kind)) {
            RenderError::RendererUnavailable { tool, hint } => {
                assert_eq!(tool, "typst");
                assert_eq!(hint, "install typst; it must be on PATH");
            }
            other => panic!("expected RendererUnavailable for {kind:?}, got {other:?}"),
        }
    }

    #[test]
    fn spawn_failure_reports_not_found_as_unavailable() {
        assert_unavailable_for(std::io::ErrorKind::NotFound);
    }

    #[test]
    fn spawn_failure_reports_permission_denied_as_unavailable() {
        // A `PATH` search that met only an unsearchable directory, or a file
        // or directory of that name it cannot execute.
        assert_unavailable_for(std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn spawn_failure_reports_not_a_directory_as_unavailable() {
        // Linux ends a `PATH` search whose last entry is a file this way.
        assert_unavailable_for(std::io::ErrorKind::NotADirectory);
    }

    #[test]
    fn spawn_failure_keeps_any_other_cause() {
        let err = spawn_failure("typst", std::io::Error::other("no-such-failure"));
        match err {
            RenderError::Spawn { program, source } => {
                assert_eq!(program, "typst");
                assert_eq!(source.to_string(), "no-such-failure");
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }
}
