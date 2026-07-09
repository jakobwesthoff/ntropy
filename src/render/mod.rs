// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Rendering a note to an output artifact such as a PDF (ADR 0038,
//! `docs/design/rendering.md`).
//!
//! The library owns the whole engine abstraction while performing no ambient
//! effect itself (ADR 0013): an engine expresses its work through a
//! [`RenderContext`] capability the host hands in, so staging files and
//! spawning tools stay in the binary. A [`Renderer`] turns a lossless
//! [`PreparedDocument`] into an artifact by driving that context; the
//! [`registry`] maps a user-facing format to the engine that produces it.
//!
//! Preparation stays lossless; each engine owns the output-shaped, lossy
//! choices. This module carries only the shared vocabulary those layers speak.

use std::ffi::OsString;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::id::Id;

pub mod prepare;
pub mod registry;

pub use prepare::prepare;
pub use registry::{DEFAULT_FORMAT, Registry};

// =========================================================
// The host capability an engine drives
// =========================================================

/// Everything an engine may ask the host to do while rendering.
///
/// The library defines the capability; the binary supplies the one production
/// implementation (temporary-directory staging plus `std::process::Command`),
/// and tests hand in a recording fake. The context grows a primitive only when
/// a real engine needs it; v1 needs exactly these three.
pub trait RenderContext {
    /// Materialize an intermediate file in a render-scoped workspace and return
    /// its path, so a later step can hand that path to an external tool.
    fn stage_file(&mut self, name: &str, contents: &[u8]) -> Result<PathBuf, RenderError>;

    /// Execute an external tool and return its captured output.
    fn run(&mut self, invocation: &Invocation) -> Result<ToolOutput, RenderError>;

    /// The path the final artifact must land at.
    fn output_path(&self) -> &Path;
}

/// An engine: a strategy that turns a [`PreparedDocument`] into an artifact by
/// driving a [`RenderContext`].
pub trait Renderer {
    /// Render `doc`, requesting every effect through `ctx`. The artifact is
    /// expected at `ctx.output_path()` when this returns `Ok`.
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError>;
}

/// A single external-tool call: the program and its argument vector.
///
/// Arguments are `OsString` because a note's title or tags may carry bytes that
/// are not valid UTF-8 on the host platform, and they pass to the tool
/// verbatim with no shell-quoting layer in between.
#[derive(Debug, Clone)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<OsString>,
}

/// The captured result of an external-tool call.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

// =========================================================
// The lossless document an engine consumes
// =========================================================

/// An engine-agnostic, lossless view of a note and its vault context.
///
/// The guardrail for growing this type: a field must be a fact about the note
/// or its vault context, resolvable without knowing the output format. Anything
/// that discards information or shapes it for output belongs to an engine, so
/// different engines can materialize the same facts differently.
#[derive(Debug, Clone)]
pub struct PreparedDocument {
    pub id: Id,
    pub path: PathBuf,
    pub title: String,
    pub tags: Vec<String>,
    /// The creation date derived from the ULID, rendered in the system-local
    /// timezone (ADR 0010).
    pub created: String,
    /// The full frontmatter mapping: the lifted fields and every other field
    /// alike, carried losslessly.
    pub frontmatter: serde_yaml_ng::Mapping,
    /// The raw body, verbatim.
    pub body: String,
    pub links: Vec<ResolvedLink>,
}

/// A note-to-note link resolved against the vault (ADR 0028).
///
/// The link table stays lossless: it carries the span and display text an
/// engine needs to substitute the link, plus the target's current title where
/// the id resolves, without committing to any particular materialization.
#[derive(Debug, Clone)]
pub struct ResolvedLink {
    /// The link's byte span in [`PreparedDocument::body`].
    pub range: Range<usize>,
    /// The link's display text.
    pub display: String,
    /// The target note's identity.
    pub id: Id,
    /// The target note's current title, or `None` when the id does not resolve
    /// against the vault (a dangling link).
    pub target_title: Option<String>,
}

// =========================================================
// Rendering failures
// =========================================================

/// A failure selecting an engine, staging a file, or running a tool.
///
/// The variants are user-facing: an unknown format or engine names the offending
/// value rather than silently substituting a different engine, and a missing
/// tool names what to install.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The requested output format is not registered.
    #[error("unknown output format `{0}`")]
    UnknownFormat(String),

    /// The requested engine does not produce the requested format. An engine
    /// registered for a different format is still unknown here.
    #[error("unknown engine `{engine}` for format `{format}`")]
    UnknownEngine { format: String, engine: String },

    /// An engine's external tool is not installed. Never a fallback to another
    /// engine (ADR 0038); the render fails naming what to install.
    #[error("required tool `{tool}` is not available: {hint}")]
    RendererUnavailable { tool: String, hint: String },

    /// Writing an intermediate file into the render workspace failed.
    #[error("while staging file `{name}`")]
    Stage {
        name: String,
        #[source]
        source: io::Error,
    },

    /// Launching the external tool failed for a reason other than absence.
    #[error("while spawning `{program}`")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },

    /// The external tool ran but exited non-zero; its stderr is surfaced so the
    /// user sees the tool's own diagnostic.
    #[error("`{program}` failed:\n{stderr}")]
    ToolFailed { program: String, stderr: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every [`RenderError`] Display string is user-facing, so each is pinned.
    #[test]
    fn unknown_format_message() {
        let err = RenderError::UnknownFormat("docx".to_string());
        insta::assert_snapshot!(err, @"unknown output format `docx`");
    }

    #[test]
    fn unknown_engine_message() {
        let err = RenderError::UnknownEngine {
            format: "pdf".to_string(),
            engine: "wkhtml".to_string(),
        };
        insta::assert_snapshot!(err, @"unknown engine `wkhtml` for format `pdf`");
    }

    #[test]
    fn renderer_unavailable_message() {
        let err = RenderError::RendererUnavailable {
            tool: "pandoc".to_string(),
            hint: "install pandoc and typst".to_string(),
        };
        insta::assert_snapshot!(err, @"required tool `pandoc` is not available: install pandoc and typst");
    }

    #[test]
    fn stage_message() {
        let err = RenderError::Stage {
            name: "note.md".to_string(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        };
        insta::assert_snapshot!(err, @"while staging file `note.md`");
    }

    #[test]
    fn spawn_message() {
        let err = RenderError::Spawn {
            program: "pandoc".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "missing"),
        };
        insta::assert_snapshot!(err, @"while spawning `pandoc`");
    }

    #[test]
    fn tool_failed_message() {
        let err = RenderError::ToolFailed {
            program: "pandoc".to_string(),
            stderr: "typst: page overflow".to_string(),
        };
        insta::assert_snapshot!(err, @r"
        `pandoc` failed:
        typst: page overflow
        ");
    }

    /// A [`RenderError`] folds into the crate [`Error`] through `#[from]`, so
    /// engines propagate it through the single crate `Result`.
    #[test]
    fn folds_into_crate_error() {
        fn fails() -> crate::error::Result<()> {
            Err(RenderError::UnknownFormat("docx".to_string()))?;
            Ok(())
        }
        let err = fails().expect_err("the conversion yields an error");
        insta::assert_snapshot!(err, @"unknown output format `docx`");
        assert!(matches!(err, crate::error::Error::Render(_)));
    }
}
