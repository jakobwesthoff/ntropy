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
use crate::render::{PreparedDocument, RenderContext, RenderError, Renderer};

/// The ntropy-owned engine registered as `typst`.
///
/// The engine builds one document from the note and delivers it according to
/// its format: the `typst` format writes the document itself, the `pdf` format
/// compiles it with the external tool.
pub struct Typst {
    format: Format,
}

/// Which artifact the engine produces. Both formats emit the identical
/// document and differ only in delivery, so the format is the one piece of
/// per-registration state the engine carries.
enum Format {
    /// The emitted Typst document, written out as the artifact directly.
    Typst,
    // TODO(pdf format): a `Pdf` variant compiles the same document through the
    // external `typst` binary; it slots in when the pdf pipeline lands.
}

impl Typst {
    /// The engine that produces the `typst` format: the emitted document written
    /// out directly as the artifact.
    pub fn for_typst_format() -> Self {
        Typst {
            format: Format::Typst,
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
        let document = document::assemble(&doc.title, &doc.frontmatter, &body);

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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;
    use crate::render::{Invocation, ResolvedLink, ToolOutput};

    use std::ops::Range;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const TARGET_ULID: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// A [`RenderContext`] that records warnings and the written artifact, so
    /// the whole engine sequence is observable without any filesystem or tool.
    /// `stage_file` and `run` are unreachable for the typst engine and panic if
    /// ever called, catching an accidental external dependency.
    struct FakeContext {
        output: PathBuf,
        warnings: Vec<String>,
        written: Option<Vec<u8>>,
    }

    impl FakeContext {
        fn new() -> Self {
            FakeContext {
                output: PathBuf::from("/artifacts/note.typ"),
                warnings: Vec::new(),
                written: None,
            }
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
            panic!("the typst format writes the artifact itself and stages nothing");
        }

        fn run(&mut self, _invocation: &Invocation) -> Result<ToolOutput, RenderError> {
            panic!("the typst format runs no external tool");
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
        Typst::for_typst_format()
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
    }

    #[test]
    fn raw_html_forwards_a_warning() {
        // Raw HTML cannot be carried faithfully, so the emitter drops it and the
        // engine forwards the warning to the host.
        let document = doc("HTML", "{}", "<div>raw</div>\n", Vec::new());
        let mut ctx = FakeContext::new();
        Typst::for_typst_format()
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
        Typst::for_typst_format()
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let written = ctx.document();
        assert!(
            written.contains("#emph[Current Title]"),
            "resolved title not emphasized: {written}"
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
        Typst::for_typst_format()
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
}
