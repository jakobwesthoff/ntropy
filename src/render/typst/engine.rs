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
    Invocation, PreparedDocument, RenderContext, RenderError, RenderOptions, Renderer, Theme,
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
    /// The vault's theme for this render, spliced after the prelude, or `None`
    /// for the built-in look (ADR 0045).
    theme: Option<Theme>,
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
    pub fn for_typst_format(options: RenderOptions, theme: Option<Theme>) -> Self {
        Typst {
            format: Format::Typst,
            options,
            theme,
        }
    }

    /// The engine that produces the `pdf` format: the identical emitted document
    /// compiled through the external `typst` binary.
    pub fn for_pdf_format(options: RenderOptions, theme: Option<Theme>) -> Self {
        Typst {
            format: Format::Pdf,
            options,
            theme,
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
        // The compiler's root is the vault, so the note's own directory is
        // where its relative asset paths must be anchored (ADR 0045).
        let asset_base = asset_base(doc);
        let (body, warnings) = emitter::emit_for_note(
            &doc.body,
            &doc.links,
            &asset_base,
            Some(&doc.title),
            Some(doc.id),
        );
        let document = document::assemble(
            &doc.title,
            &doc.frontmatter,
            self.options.paper,
            self.theme.as_ref().map(|theme| theme.source.as_str()),
            &body,
        );

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
            // input argument is `-` and places at the compile root. A relative
            // path would therefore resolve against the vault root, which is why
            // the emitter writes local image paths root-absolute, leaving only a
            // path that climbs out of the vault as written (the "Asset paths"
            // contract of `docs/design/typst-engine.md`). The output path
            // reaches typst as an argument verbatim; it is absolute (see
            // `RenderContext::output_path`). The engine itself touches neither
            // the filesystem nor the working directory.
            Format::Pdf => {
                // `--root` grants the compiler the whole vault, so a theme can
                // reach assets outside `all-notes/` by a vault-absolute path
                // like `/assets/logo.svg` (ADR 0045). It is passed on every
                // render, themed or not, so one invocation shape covers both.
                let args: Vec<std::ffi::OsString> = vec![
                    "compile".into(),
                    "--root".into(),
                    doc.vault_root.as_os_str().to_os_string(),
                    "-".into(),
                    ctx.output_path().as_os_str().to_os_string(),
                ];
                let document = document.into_bytes();
                let invocation = Invocation {
                    program: "typst".to_string(),
                    args,
                    stdin: Some(document.clone()),
                };

                let out = ctx.run(&invocation)?;
                if !out.success {
                    return Err(RenderError::ToolFailed {
                        program: "typst".to_string(),
                        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                    });
                }

                // The warnings of the compile reach the user before the table
                // check runs: that run compiles the same document again and
                // prints the same warnings, which is why only the compile's
                // stderr is read.
                for warning in typst_warnings(&String::from_utf8_lossy(&out.stderr)) {
                    ctx.warn(&warning);
                }

                // Only a table can be too wide, so a note without one pays for
                // no second run of the compiler.
                if body.contains("#table(") {
                    warn_about_tables_that_do_not_fit(ctx, &doc.vault_root, document);
                }
                Ok(())
            }
        }
    }
}

/// Label the prelude puts on the record of a table it had to set in text
/// smaller than it allows itself, see the table rule in `prelude.typ`.
const TABLE_TOO_WIDE_LABEL: &str = "<ntropy-table-too-wide>";

/// The warnings in the stderr of a `typst compile` that succeeded, each as
/// one line for the host.
///
/// typst prints a warning as a block: a `warning: <message>` line, then
/// optionally a snippet of the source with its location above it, then
/// `= hint:` lines whose continuation lines are indented under them. A block
/// ends at the next `warning:` or `error:` line at the start of a line, which
/// also keeps the source lines of an error out of a warning. Anything else on
/// stderr is not a warning and is dropped.
///
/// The location is left out. The document typst compiles is the one ntropy
/// assembled from prelude, theme and note, so `<stdin>:10:16` points into a
/// text the user never sees. The quoted source line identifies the spot
/// instead.
fn typst_warnings(stderr: &str) -> Vec<String> {
    struct Block {
        message: String,
        source: Option<String>,
        hints: Vec<String>,
        in_hint: bool,
    }

    fn finish(block: Block, warnings: &mut Vec<String>) {
        let mut line = block.message;
        if let Some(source) = block.source {
            line.push_str(&format!(" (in `{source}`)"));
        }
        if !block.hints.is_empty() {
            line.push_str(&format!(" (hint: {})", block.hints.join("; ")));
        }
        warnings.push(line);
    }

    let mut warnings = Vec::new();
    let mut current: Option<Block> = None;
    for line in stderr.lines() {
        if let Some(message) = line.strip_prefix("warning: ") {
            if let Some(block) = current.take() {
                finish(block, &mut warnings);
            }
            current = Some(Block {
                message: message.trim().to_string(),
                source: None,
                hints: Vec::new(),
                in_hint: false,
            });
            continue;
        }
        if line.starts_with("error: ") {
            if let Some(block) = current.take() {
                finish(block, &mut warnings);
            }
            continue;
        }
        let Some(block) = current.as_mut() else {
            continue;
        };

        let text = line.trim();
        if let Some(hint) = text.strip_prefix("= hint: ") {
            block.hints.push(hint.trim().to_string());
            block.in_hint = true;
            continue;
        }

        // A source line is `<number> │ <code>`; the lines of the gutter around
        // it carry no number.
        let numbered_source = text
            .split_once('│')
            .filter(|(number, _)| {
                !number.is_empty() && number.trim().chars().all(|c| c.is_ascii_digit())
            })
            .map(|(_, code)| code.trim().to_string());
        if let Some(code) = numbered_source {
            block.source.get_or_insert(code);
            block.in_hint = false;
        } else if text.is_empty()
            || text.starts_with('│')
            || text.starts_with('┌')
            || text.starts_with("= ")
        {
            block.in_hint = false;
        } else if block.in_hint {
            let hint = block.hints.last_mut().expect("a hint is open");
            hint.push(' ');
            hint.push_str(text);
        }
    }
    if let Some(block) = current {
        finish(block, &mut warnings);
    }
    warnings
}

/// Ask the compiler which tables of `document` do not fit the page at the
/// smallest text size the prelude allows, and warn about each one.
///
/// The prelude decides this with the measured widths of the cells, which only
/// the compiler can know, so the answer comes from a second run on the same
/// document. A check that cannot run or cannot be read is a warning of its own
/// and leaves the finished PDF alone. The run's stderr is not parsed for
/// warnings: it repeats those of the compile.
fn warn_about_tables_that_do_not_fit(
    ctx: &mut dyn RenderContext,
    vault_root: &std::path::Path,
    document: Vec<u8>,
) {
    let invocation = Invocation {
        program: "typst".to_string(),
        args: vec![
            "eval".into(),
            "--root".into(),
            vault_root.as_os_str().to_os_string(),
            "--in".into(),
            "-".into(),
            format!("query({TABLE_TOO_WIDE_LABEL}).map(m => m.value)").into(),
        ],
        stdin: Some(document),
    };
    let cannot_check =
        |cause: &str| format!("could not check whether the tables fit the page: {cause}");

    let out = match ctx.run(&invocation) {
        Ok(out) => out,
        Err(error) => {
            ctx.warn(&cannot_check(&error.to_string()));
            return;
        }
    };
    if !out.success {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let cause = stderr
            .lines()
            .find_map(|line| line.strip_prefix("error: "))
            .unwrap_or_else(|| stderr.trim());
        ctx.warn(&cannot_check(cause));
        return;
    }

    #[derive(serde::Deserialize)]
    struct Clamped {
        columns: u32,
        scale: f64,
    }
    let clamped: Vec<Clamped> = match serde_json::from_slice(&out.stdout) {
        Ok(clamped) => clamped,
        Err(error) => {
            ctx.warn(&cannot_check(&error.to_string()));
            return;
        }
    };
    for table in clamped {
        ctx.warn(&format!(
            "a table with {} columns needs the text at {}% of its size to fit the page; \
             it is set at 60% and runs past the right edge of the page, so columns are cut off",
            table.columns,
            (table.scale * 100.0).round()
        ));
    }
}

/// The note's directory expressed as a path absolute within the compile root,
/// e.g. `/all-notes` for a note in a standard vault.
///
/// Falls back to the root itself when the note does not sit under it, which no
/// vault produces; the emitted paths are then root-relative to the root, and
/// the compiler reports anything it cannot find.
fn asset_base(doc: &PreparedDocument) -> String {
    let dir = match doc.path.parent() {
        Some(dir) => dir,
        None => return "/".to_string(),
    };
    match dir.strip_prefix(&doc.vault_root) {
        Ok(relative) => {
            let text = relative.to_string_lossy();
            if text.is_empty() {
                "/".to_string()
            } else {
                format!("/{text}")
            }
        }
        Err(_) => "/".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;
    use crate::render::{LinkTarget, ResolvedLink, ToolOutput};

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

        fn write_file(&mut self, _relative: &str, _contents: &[u8]) -> Result<(), RenderError> {
            panic!("the typst engine writes nothing beside the artifact");
        }

        fn copy_file(&mut self, _from: &Path, _relative: &str) -> Result<(), RenderError> {
            panic!("the typst engine copies nothing beside the artifact");
        }

        fn warn(&mut self, message: &str) {
            self.warnings.push(message.to_string());
        }

        fn output_path(&self) -> &Path {
            &self.output
        }
    }

    fn link(range: Range<usize>, display: &str, target: Option<(&str, &str)>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: display.to_string(),
            id: Id::from_str(TARGET_ULID).expect("target ulid parses"),
            anchored: false,
            target: target.map(|(title, slug)| LinkTarget {
                title: title.to_string(),
                slug: slug.to_string(),
            }),
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
            vault_root: PathBuf::from("/vault"),
            title: title.to_string(),
            tags: Vec::new(),
            created: "2020-01-01".to_string(),
            frontmatter: serde_yaml_ng::from_str(frontmatter)
                .expect("the fixture frontmatter parses into a mapping"),
            frontmatter_source: frontmatter.to_string(),
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
        Typst::for_typst_format(RenderOptions::default(), None)
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
        Typst::for_typst_format(RenderOptions::default(), None)
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
    fn resolved_note_link_renders_the_title_linked_to_the_targets_artifact() {
        // A resolved note link becomes the target's styled title wrapped in a
        // link to the target's own artifact; the display text does not appear.
        let body = "see [old](note.md) here".to_string();
        let start = body.find("[old]").expect("link present");
        let end = start + "[old](note.md)".len();
        let links = vec![link(
            start..end,
            "old",
            Some(("Current Title", "current-title")),
        )];
        let document = doc("Links", "{}", &body, links);

        let mut ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default(), None)
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let written = ctx.document();
        assert!(
            written.contains(r#"#link("current-title.pdf")[#notelink[Current Title]]"#),
            "resolved note link not linked to the target's artifact: {written}"
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
        Typst::for_typst_format(RenderOptions::default(), None)
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
        // and targets the (absolute) output path.
        let document = doc(
            "My Note",
            "tags:\n  - area/work\n",
            "# Heading\n\nSome *body* text.\n",
            Vec::new(),
        );

        let mut typst_ctx = FakeContext::new();
        Typst::for_typst_format(RenderOptions::default(), None)
            .render(&document, &mut typst_ctx)
            .expect("typst-format render succeeds");
        let expected_document = typst_ctx
            .written
            .clone()
            .expect("the typst format wrote the document");

        let mut ctx = FakeContext::new().with_output("/artifacts/note.pdf");
        Typst::for_pdf_format(RenderOptions::default(), None)
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
        assert_eq!(
            args,
            vec!["compile", "--root", "/vault", "-", "/artifacts/note.pdf"],
            "the vault root is granted to the compiler so a theme reaches its assets"
        );
        assert_eq!(
            invocation.stdin.as_deref(),
            Some(expected_document.as_slice()),
            "stdin carries the exact bytes the typst format writes"
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

        let err = Typst::for_pdf_format(RenderOptions::default(), None)
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

    /// A note whose body holds one table, so the pdf arm asks typst which
    /// tables the prelude had to clamp.
    fn table_document() -> PreparedDocument {
        doc(
            "Wide",
            "{}",
            "| a | b |\n| - | - |\n| 1 | 2 |\n",
            Vec::new(),
        )
    }

    fn tool_output(success: bool, stdout: &str, stderr: &str) -> Result<ToolOutput, RenderError> {
        Ok(ToolOutput {
            success,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    fn pdf_context() -> FakeContext {
        FakeContext::new().with_output("/artifacts/note.pdf")
    }

    #[test]
    fn a_note_without_a_table_asks_typst_nothing_after_the_compile() {
        let document = doc("Plain", "{}", "Some text.\n", Vec::new());
        let mut ctx = pdf_context();
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&document, &mut ctx)
            .expect("render succeeds");
        assert_eq!(ctx.invocations.len(), 1, "only the compile runs");
    }

    #[test]
    fn a_table_makes_the_pdf_arm_query_the_clamped_tables() {
        let mut ctx = pdf_context()
            .script(tool_output(true, "", ""))
            .script(tool_output(true, "[]", ""));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("render succeeds");

        assert_eq!(ctx.invocations.len(), 2, "the compile and the query");
        let query = &ctx.invocations[1];
        assert_eq!(query.program, "typst");
        let args: Vec<String> = query
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "eval",
                "--root",
                "/vault",
                "--in",
                "-",
                "query(<ntropy-table-too-wide>).map(m => m.value)"
            ],
            "the query reads the same document from stdin"
        );
        assert_eq!(
            query.stdin, ctx.invocations[0].stdin,
            "the query sees the document the compile saw"
        );
        assert!(ctx.warnings.is_empty(), "no clamped table, no warning");
    }

    #[test]
    fn a_table_clamped_at_the_floor_warns_with_its_columns_and_scale() {
        let mut ctx = pdf_context()
            .script(tool_output(true, "", ""))
            .script(tool_output(
                true,
                r#"[{"columns":16,"scale":0.5073},{"columns":12,"scale":0.4}]"#,
                "",
            ));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("a clamped table is a warning, not a failure");

        assert_eq!(
            ctx.warnings,
            vec![
                "a table with 16 columns needs the text at 51% of its size to fit the page; \
                 it is set at 60% and runs past the right edge of the page, so columns are cut off",
                "a table with 12 columns needs the text at 40% of its size to fit the page; \
                 it is set at 60% and runs past the right edge of the page, so columns are cut off",
            ],
            "each clamped table is one warning"
        );
    }

    #[test]
    fn a_failed_query_warns_about_the_check_and_keeps_the_render() {
        let mut ctx = pdf_context()
            .script(tool_output(true, "", ""))
            .script(tool_output(
                false,
                "",
                "warning: unknown font family: nope\nerror: unknown subcommand\n  ┌─ <stdin>:1:1\n",
            ));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("a failed check does not fail the render");

        assert_eq!(ctx.warnings.len(), 1);
        assert!(
            ctx.warnings[0].contains("could not check whether the tables fit the page")
                && ctx.warnings[0].contains("unknown subcommand"),
            "the warning names the check and the cause: {:?}",
            ctx.warnings
        );
    }

    #[test]
    fn an_unreadable_query_answer_warns_about_the_check() {
        let mut ctx = pdf_context()
            .script(tool_output(true, "", ""))
            .script(tool_output(true, "not json", ""));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("an unreadable answer does not fail the render");

        assert_eq!(ctx.warnings.len(), 1);
        assert!(
            ctx.warnings[0].contains("could not check whether the tables fit the page"),
            "{:?}",
            ctx.warnings
        );
    }

    #[test]
    fn a_query_that_cannot_start_warns_about_the_check() {
        let mut ctx = pdf_context().script(tool_output(true, "", "")).script(Err(
            RenderError::RendererUnavailable {
                tool: "typst".to_string(),
                hint: "install typst".to_string(),
            },
        ));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("a check that cannot run does not fail the render");
        assert_eq!(ctx.warnings.len(), 1);
    }

    #[test]
    fn a_failed_compile_runs_no_query() {
        let mut ctx = pdf_context().script(tool_output(false, "", "error: broken"));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect_err("the compile failed");
        assert_eq!(ctx.invocations.len(), 1);
    }

    // Verbatim `typst compile` stderr (typst 0.15.1) of documents that compile.
    const UNKNOWN_FONT: &str = "warning: unknown font family: nope
  ┌─ <stdin>:1:16
  │
1 │ #set text(font: \"Nope\")
  │                 ^^^^^^

";
    const UNKNOWN_FONT_LINE_TEN: &str = "warning: unknown font family: nope
   ┌─ <stdin>:10:16
   │
10 │ #set text(font: \"Nope\")
   │                 ^^^^^^

";
    const NOT_CONVERGED: &str = "warning: document did not converge within five attempts
 = hint: see 2 additional warnings for more details
 = hint: see https://typst.app/help/convergence for help

warning: value of `state(\"x\")` did not converge
  ┌─ conv.typ:3:33
  │
3 │ #context [#s.update(x => x + 1) #s.final()]
  │                                  ^^^^^^^^^
  │
  = hint: the following values were observed:
    - run 1: `0`
    - run 2: `1`
    - final: `5`
  = hint: see https://typst.app/help/state-convergence for help

";

    fn warnings_of_a_compile(stderr: &str) -> Vec<String> {
        let mut ctx = pdf_context().script(tool_output(true, "", stderr));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&doc("Plain", "{}", "Some text.\n", Vec::new()), &mut ctx)
            .expect("a warning does not fail the render");
        ctx.warnings
    }

    #[test]
    fn a_compile_warning_reaches_the_host_with_its_source_line() {
        assert_eq!(
            warnings_of_a_compile(UNKNOWN_FONT),
            vec![r#"unknown font family: nope (in `#set text(font: "Nope")`)"#]
        );
    }

    #[test]
    fn a_compile_that_prints_nothing_warns_nothing() {
        assert!(warnings_of_a_compile("").is_empty());
    }

    #[test]
    fn several_compile_warnings_become_one_warning_each_with_their_hints() {
        assert_eq!(
            warnings_of_a_compile(NOT_CONVERGED),
            vec![
                "document did not converge within five attempts (hint: see 2 additional \
                 warnings for more details; see https://typst.app/help/convergence for help)",
                "value of `state(\"x\")` did not converge \
                 (in `#context [#s.update(x => x + 1) #s.final()]`) \
                 (hint: the following values were observed: - run 1: `0` - run 2: `1` \
                 - final: `5`; see https://typst.app/help/state-convergence for help)",
            ]
        );
    }

    #[test]
    fn a_source_line_with_a_wider_gutter_is_still_found() {
        assert_eq!(
            warnings_of_a_compile(UNKNOWN_FONT_LINE_TEN),
            vec![r#"unknown font family: nope (in `#set text(font: "Nope")`)"#]
        );
    }

    #[test]
    fn stderr_outside_a_warning_is_dropped() {
        let stderr = format!("compiling...\nnote: something\n{UNKNOWN_FONT}done\n");
        assert_eq!(
            warnings_of_a_compile(&stderr),
            vec![r#"unknown font family: nope (in `#set text(font: "Nope")`)"#]
        );
    }

    #[test]
    fn a_warning_block_ends_at_an_error_line() {
        let stderr = format!(
            "error: broken\n  ┌─ <stdin>:2:1\n  │\n2 │ #oops\n  │ ^^^^^\n\n{UNKNOWN_FONT}\
             error: late\n  ┌─ <stdin>:3:1\n  │\n3 │ #late\n  │ ^^^^^\n"
        );
        assert_eq!(
            warnings_of_a_compile(&stderr),
            vec![r#"unknown font family: nope (in `#set text(font: "Nope")`)"#],
            "the source lines of the errors do not leak into the warning"
        );
    }

    #[test]
    fn the_table_warning_follows_the_compile_warnings_and_the_query_stderr_is_ignored() {
        let mut ctx = pdf_context()
            .script(tool_output(true, "", UNKNOWN_FONT))
            .script(tool_output(
                true,
                r#"[{"columns":16,"scale":0.5}]"#,
                UNKNOWN_FONT,
            ));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("warnings do not fail the render");
        assert_eq!(
            ctx.warnings,
            vec![
                r#"unknown font family: nope (in `#set text(font: "Nope")`)"#.to_string(),
                "a table with 16 columns needs the text at 50% of its size to fit the page; \
                 it is set at 60% and runs past the right edge of the page, so columns are cut off"
                    .to_string(),
            ],
            "the font warning is reported once although the query compiles the document again"
        );
    }

    #[test]
    fn a_failed_table_check_names_the_first_error_line_as_its_cause() {
        let mut ctx = pdf_context()
            .script(tool_output(true, "", ""))
            .script(tool_output(
                false,
                "",
                "warning: unknown font family: nope\nerror: file not found\n  ┌─ <stdin>:1:1\n",
            ));
        Typst::for_pdf_format(RenderOptions::default(), None)
            .render(&table_document(), &mut ctx)
            .expect("a failed check does not fail the render");
        assert_eq!(
            ctx.warnings,
            vec!["could not check whether the tables fit the page: file not found"]
        );
    }

    #[test]
    fn configured_paper_reaches_the_document_in_both_formats() {
        use crate::render::{Paper, RenderOptions};

        let options = RenderOptions {
            paper: Paper::UsLetter,
            theme: None,
        };
        let document = doc("Paper", "{}", "body\n", Vec::new());

        // The typst format carries the paper in the written artifact.
        let mut typst_ctx = FakeContext::new();
        Typst::for_typst_format(options.clone(), None)
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
        Typst::for_pdf_format(options, None)
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
