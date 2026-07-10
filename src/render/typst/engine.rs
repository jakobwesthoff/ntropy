// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The engine that ties the emitter, document assembly, and the host context
//! together (ADR 0040, `docs/design/typst-engine.md`).
//!
//! One assembled document serves both output formats: the `typst` format hands
//! it out as the artifact, and the `pdf` format compiles that identical
//! document through the external `typst` binary. The engine therefore carries
//! which format it is producing, so the same emitted bytes reach the host
//! either as a written file or as a compiler invocation.

use super::{document, emitter};
use crate::render::{
    Invocation, PreparedDocument, RenderContext, RenderError, RenderOptions, Renderer,
};

/// The ntropy-owned engine registered as `typst`.
///
/// The engine builds one document from the note and delivers it according to
/// its format: the `typst` format writes the document itself, the `pdf` format
/// compiles it with the external tool. The render options travel into the
/// emitted document (the paper as the template's `paper:` argument), so the
/// `typst`-format artifact carries everything needed to compile identically.
pub struct Typst {
    format: Format,
    options: RenderOptions,
}

/// Which artifact the engine produces. Both formats emit the identical
/// document and differ only in delivery, so the format and the render options
/// are the per-registration state the engine carries.
enum Format {
    /// The emitted Typst document, written out as the artifact directly.
    Typst,
    /// A PDF: the identical emitted document compiled through the external
    /// `typst` binary.
    Pdf,
}

impl Typst {
    /// The engine that produces the `typst` format: the emitted document written
    /// out directly as the artifact.
    pub fn for_typst_format(options: RenderOptions) -> Self {
        Typst {
            format: Format::Typst,
            options,
        }
    }

    /// The engine that produces the `pdf` format: the identical emitted document
    /// compiled through the external `typst` binary.
    pub fn for_pdf_format(options: RenderOptions) -> Self {
        Typst {
            format: Format::Pdf,
            options,
        }
    }
}

impl Renderer for Typst {
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError> {
        // Convert the body once; the same bytes back every format. Note links
        // resolve against the prepared link table (ADR 0028).
        let (body, warnings) = emitter::emit(&doc.body, &doc.links);
        let document = document::assemble(&doc.title, &doc.frontmatter, self.options.paper, &body);

        // Every degradation the emitter reported (dropped raw HTML, remote
        // images the offline compiler cannot embed) reaches the host, which
        // surfaces it and, under `--strict`, fails on it.
        for warning in &warnings {
            ctx.warn(&warning.message);
        }

        match self.format {
            // The document is the artifact; the host writes it out. No external
            // tool is involved, so nothing needs to be installed.
            Format::Typst => ctx.write_output(document.as_bytes()),

            // The document is compiled to a PDF by the external `typst` binary.
            // It rides on stdin, which typst reads as the main file when the
            // input argument is `-`, and the note's own directory becomes the
            // working directory so relative asset paths in the document resolve
            // against the note (the "Asset paths" contract of
            // `docs/design/typst-engine.md`). The output path reaches typst as
            // an argument verbatim; it must already be absolute, since a
            // relative path would resolve inside the moved working directory,
            // landing the artifact next to the note. Absolutizing is the host's
            // job, keeping the engine headless: it touches neither the
            // filesystem nor the working directory itself.
            Format::Pdf => {
                let args: Vec<std::ffi::OsString> = vec![
                    "compile".into(),
                    "-".into(),
                    ctx.output_path().as_os_str().to_os_string(),
                ];
                let invocation = Invocation {
                    program: "typst".to_string(),
                    args,
                    stdin: Some(document.into_bytes()),
                    cwd: doc.path.parent().map(|parent| parent.to_path_buf()),
                };

                let out = ctx.run(&invocation)?;
                if !out.success {
                    return Err(RenderError::ToolFailed {
                        program: "typst".to_string(),
                        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                    });
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;
    use crate::render::{ResolvedLink, ToolOutput};

    use std::collections::VecDeque;
    use std::ops::Range;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const TARGET_ULID: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// A [`RenderContext`] that records warnings, the written artifact, and every
    /// tool invocation, so the whole engine sequence is observable without any
    /// filesystem or real tool. `run` returns the next scripted result, or a
    /// plain success once the script is exhausted, so a test that only inspects
    /// the invocation need script nothing. `stage_file` is unreachable for the
    /// typst engine and panics if ever called, catching an accidental staging.
    struct FakeContext {
        output: PathBuf,
        warnings: Vec<String>,
        written: Option<Vec<u8>>,
        invocations: Vec<Invocation>,
        scripted: VecDeque<Result<ToolOutput, RenderError>>,
    }

    impl FakeContext {
        fn new() -> Self {
            FakeContext {
                output: PathBuf::from("/artifacts/note.typ"),
                warnings: Vec::new(),
                written: None,
                invocations: Vec::new(),
                scripted: VecDeque::new(),
            }
        }

        /// Set the artifact path the context reports, so the pdf arm's output
        /// argument is observable at a fixed, filesystem-independent location.
        fn with_output(mut self, output: &str) -> Self {
            self.output = PathBuf::from(output);
            self
        }

        /// Queue the result the next `run` call returns.
        fn script(mut self, result: Result<ToolOutput, RenderError>) -> Self {
            self.scripted.push_back(result);
            self
        }

        /// The written artifact decoded as UTF-8. The emitter only ever writes
        /// valid UTF-8, so a failure here is a test-fixture bug.
        fn document(&self) -> String {
            String::from_utf8(self.written.clone().expect("the engine wrote an artifact"))
                .expect("the artifact is valid UTF-8")
        }
    }

    impl RenderContext for FakeContext {
        fn stage_file(&mut self, _name: &str, _contents: &[u8]) -> Result<PathBuf, RenderError> {
            panic!("the typst engine writes or compiles the artifact itself and stages nothing");
        }

        fn run(&mut self, invocation: &Invocation) -> Result<ToolOutput, RenderError> {
            self.invocations.push(invocation.clone());
            self.scripted.pop_front().unwrap_or(Ok(ToolOutput {
                success: true,
                stdout: Vec::new(),
                stderr: Vec::new(),
            }))
        }

        fn write_output(&mut self, contents: &[u8]) -> Result<(), RenderError> {
            self.written = Some(contents.to_vec());
            Ok(())
        }

        fn warn(&mut self, message: &str) {
            self.warnings.push(message.to_string());
        }

        fn output_path(&self) -> &Path {
            &self.output
        }
    }

    fn link(range: Range<usize>, display: &str, target_title: Option<&str>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: display.to_string(),
            id: Id::from_str(TARGET_ULID).expect("target ulid parses"),
            target_title: target_title.map(str::to_string),
        }
    }

    /// A prepared document fixture. Tests vary title, frontmatter, body, and
    /// links; the id, path, tags, and created date stay fixed and irrelevant.
    fn doc(
        title: &str,
        frontmatter: &str,
        body: &str,
        links: Vec<ResolvedLink>,
    ) -> PreparedDocument {
        PreparedDocument {
            id: Id::from_str(ULID).expect("ulid parses"),
            path: PathBuf::from("/vault/all-notes/note.md"),
            title: title.to_string(),
            tags: Vec::new(),
            created: "2020-01-01".to_string(),
            frontmatter: serde_yaml_ng::from_str(frontmatter)
                .expect("the fixture frontmatter parses into a mapping"),
            body: body.to_string(),
            links,
        }
    }

    #[test]
    fn writes_the_assembled_document() {
        // The written artifact carries the prelude, the template application
        // with the title as a string literal, the frontmatter as a typed value,
        // and the converted body.
        let document = doc(
            "My Note",
            "tags:\n  - area/work\n",
            "# Heading\n\nSome *body* text.\n",
            Vec::new(),
        );
        let mut ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let written = ctx.document();
        assert!(written.contains("#let note("), "prelude missing: {written}");
        assert!(
            written.contains(r#"note.with(title: "My Note""#),
            "title not applied: {written}"
        );
        assert!(
            written.contains(r#""tags": ("area/work",)"#),
            "frontmatter not translated: {written}"
        );
        assert!(
            written.contains("= Heading"),
            "heading not emitted: {written}"
        );
        assert!(
            written.contains("#emph[body]"),
            "inline markup not converted: {written}"
        );
        assert!(ctx.warnings.is_empty(), "clean note warns nothing");
        assert!(
            ctx.invocations.is_empty(),
            "the typst format runs no external tool"
        );
    }

    #[test]
    fn raw_html_forwards_a_warning() {
        // Raw HTML cannot be carried faithfully, so the emitter drops it and the
        // engine forwards the warning to the host.
        let document = doc("HTML", "{}", "<div>raw</div>\n", Vec::new());
        let mut ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");

        assert!(
            !ctx.warnings.is_empty(),
            "raw HTML must forward at least one warning"
        );
        assert!(
            ctx.warnings.iter().any(|w| w.contains("HTML")),
            "the warning names the dropped HTML: {:?}",
            ctx.warnings
        );
    }

    #[test]
    fn resolved_note_link_renders_the_title() {
        // A resolved note link becomes the target's emphasized title; the
        // display text does not appear.
        let body = "see [old](note.md) here".to_string();
        let start = body.find("[old]").expect("link present");
        let end = start + "[old](note.md)".len();
        let links = vec![link(start..end, "old", Some("Current Title"))];
        let document = doc("Links", "{}", &body, links);

        let mut ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let written = ctx.document();
        assert!(
            written.contains("#notelink[Current Title]"),
            "resolved title not routed through notelink: {written}"
        );
    }

    #[test]
    fn unresolved_note_link_keeps_its_display_text() {
        // An unresolved note link drops the wrapper and re-emits its inner
        // markup, so the display text survives (escaped) in the artifact.
        let body = "see [gone](note.md) here".to_string();
        let start = body.find("[gone]").expect("link present");
        let end = start + "[gone](note.md)".len();
        let links = vec![link(start..end, "gone", None)];
        let document = doc("Links", "{}", &body, links);

        let mut ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let written = ctx.document();
        assert!(
            written.contains("gone"),
            "unresolved display text missing: {written}"
        );
        assert!(
            !written.contains(r#"#link("note.md")"#),
            "an unresolved note link must not emit a plain link: {written}"
        );
    }

    #[test]
    fn pdf_arm_builds_the_typst_compile_invocation() {
        // The pdf arm compiles the identical document the typst format writes.
        // Render the same note through both formats and prove the pdf arm feeds
        // the typst-format bytes on stdin, compiles them with `typst compile -`,
        // targets the (absolute) output path, and runs in the note's directory.
        let document = doc(
            "My Note",
            "tags:\n  - area/work\n",
            "# Heading\n\nSome *body* text.\n",
            Vec::new(),
        );

        let mut typst_ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default())
            .render(&document, &mut typst_ctx)
            .expect("typst-format render succeeds");
        let expected_document = typst_ctx
            .written
            .clone()
            .expect("the typst format wrote the document");

        let mut ctx = FakeContext::new().with_output("/artifacts/note.pdf");
        Typst::for_pdf_format(RenderOptions::default())
            .render(&document, &mut ctx)
            .expect("pdf render succeeds");

        assert!(
            ctx.written.is_none(),
            "the pdf arm delegates to the tool and writes nothing itself"
        );
        assert_eq!(ctx.invocations.len(), 1, "exactly one compile invocation");
        let invocation = &ctx.invocations[0];
        assert_eq!(invocation.program, "typst");
        let args: Vec<String> = invocation
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["compile", "-", "/artifacts/note.pdf"]);
        assert_eq!(
            invocation.stdin.as_deref(),
            Some(expected_document.as_slice()),
            "stdin carries the exact bytes the typst format writes"
        );
        assert_eq!(
            invocation.cwd.as_deref(),
            Some(Path::new("/vault/all-notes")),
            "the note's parent directory is the working directory"
        );
    }

    #[test]
    fn pdf_arm_surfaces_a_compile_failure() {
        // A non-zero exit from typst fails the render, surfacing the compiler's
        // own stderr so the user sees why the compile failed.
        let document = doc("Fails", "{}", "body\n", Vec::new());
        let mut ctx = FakeContext::new()
            .with_output("/artifacts/note.pdf")
            .script(Ok(ToolOutput {
                success: false,
                stdout: Vec::new(),
                stderr: b"error: file would escape the project root".to_vec(),
            }));

        let err = Typst::for_pdf_format(RenderOptions::default())
            .render(&document, &mut ctx)
            .expect_err("a non-zero exit fails the render");
        match err {
            RenderError::ToolFailed { program, stderr } => {
                assert_eq!(program, "typst");
                assert_eq!(stderr, "error: file would escape the project root");
            }
            other => panic!("expected ToolFailed, got {other:?}"),
        }
    }

    #[test]
    fn configured_paper_reaches_the_document_in_both_formats() {
        use crate::render::{Paper, RenderOptions};

        let options = RenderOptions {
            paper: Paper::UsLetter,
        };
        let document = doc("Paper", "{}", "body\n", Vec::new());

        // The typst format carries the paper in the written artifact.
        let mut typst_ctx = FakeContext::new();
        Typst::for_typst_format(options)
            .render(&document, &mut typst_ctx)
            .expect("render succeeds");
        assert!(
            typst_ctx.document().contains(r#"paper: "us-letter","#),
            "typst artifact misses the paper: {}",
            typst_ctx.document()
        );

        // The pdf format compiles the identical bytes, so the same paper rides
        // on the compiler's stdin.
        let mut pdf_ctx = FakeContext::new().with_output("/artifacts/note.pdf");
        Typst::for_pdf_format(options)
            .render(&document, &mut pdf_ctx)
            .expect("render succeeds");
        let stdin = pdf_ctx.invocations[0]
            .stdin
            .clone()
            .expect("the pdf arm pipes the document");
        assert!(
            String::from_utf8(stdin)
                .expect("the document is UTF-8")
                .contains(r#"paper: "us-letter","#),
            "pdf stdin misses the paper"
        );
    }
}
