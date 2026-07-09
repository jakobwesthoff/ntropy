// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The v1 `pdf` engine: pandoc reading GitHub-flavored Markdown, delegating PDF
//! typesetting to typst (`docs/design/rendering.md`, ADR 0038).
//!
//! This engine owns the lossy half of rendering. It flattens the lossless
//! [`PreparedDocument`] into a Markdown file plus a metadata argument vector,
//! then hands pandoc the single invocation that produces the artifact. All
//! effects travel through the [`RenderContext`], so the engine spawns nothing
//! itself and the exact invocation is snapshot-testable without pandoc present.

use super::{Invocation, PreparedDocument, RenderContext, RenderError, Renderer};

/// The pandoc-plus-typst engine registered as the default `pdf` producer.
pub struct Pandoc;

impl Renderer for Pandoc {
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError> {
        // Flatten the body to Markdown, then stage it under a fixed name so the
        // invocation that references it is deterministic.
        let materialized = materialize_body(doc);
        let staged = ctx.stage_file("note.md", materialized.as_bytes())?;

        // Assemble the invocation exactly as the design spec pins it. Metadata
        // values pass verbatim as single argv entries: there is no shell between
        // this and the process spawn, so a value carrying `=`, quotes, or `·`
        // needs no quoting and keeps its bytes.
        let mut args: Vec<std::ffi::OsString> = vec![
            staged.into_os_string(),
            "--from".into(),
            "gfm".into(),
            "--pdf-engine=typst".into(),
            "--metadata".into(),
            format!("title={}", doc.title).into(),
            "--metadata".into(),
            format!("date={}", doc.created).into(),
        ];

        // Tags are typeset as the subtitle, which pandoc's typst template
        // treats as escaped content. They are deliberately not passed as
        // `keywords`: the stock template splices that value verbatim into typst
        // code, where any plain string fails to compile. With no tags there is
        // nothing to typeset, so the entry is dropped rather than emitted empty.
        if !doc.tags.is_empty() {
            let subtitle = doc
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" · ");
            args.push("--metadata".into());
            args.push(format!("subtitle={subtitle}").into());
        }

        args.push("--output".into());
        args.push(ctx.output_path().as_os_str().to_os_string());

        let invocation = Invocation {
            program: "pandoc".to_string(),
            args,
        };

        let out = ctx.run(&invocation)?;
        if !out.success {
            return Err(RenderError::ToolFailed {
                program: "pandoc".to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        Ok(())
    }
}

/// Flatten the body into the Markdown pandoc reads, substituting every link.
///
/// A resolved link becomes the target's current title as emphasized text
/// (`*Title*`); an unresolved link collapses to its bare display text. Spans are
/// rewritten from the last to the first so each replacement leaves the byte
/// offsets of the not-yet-processed, earlier spans untouched.
fn materialize_body(doc: &PreparedDocument) -> String {
    let mut body = doc.body.clone();

    // The prepared table follows extraction order; sorting by descending start
    // makes the back-to-front rewrite independent of that order.
    let mut links: Vec<&super::ResolvedLink> = doc.links.iter().collect();
    links.sort_by_key(|link| std::cmp::Reverse(link.range.start));

    for link in links {
        let replacement = match &link.target_title {
            Some(title) => format!("*{title}*"),
            None => link.display.clone(),
        };
        body.replace_range(link.range.clone(), &replacement);
    }

    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{ResolvedLink, ToolOutput};

    use std::collections::VecDeque;
    use std::ops::Range;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    use crate::id::Id;

    // A fixed identity keeps `PreparedDocument::id` deterministic in fixtures.
    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    // A distinct identity for link targets.
    const TARGET_ULID: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

    /// A [`RenderContext`] that stages into memory and scripts `run` outputs.
    ///
    /// It records every staged file and every invocation so the whole engine
    /// sequence can be snapshot, and returns fixed paths so those snapshots do
    /// not depend on any real filesystem location. `run` returns the next
    /// scripted result, or a plain success once the script is exhausted, so a
    /// test that only cares about the argv need script nothing.
    struct FakeContext {
        output: PathBuf,
        staged: Vec<(String, String)>,
        invocations: Vec<Invocation>,
        scripted: VecDeque<Result<ToolOutput, RenderError>>,
    }

    impl FakeContext {
        fn new() -> Self {
            FakeContext {
                output: PathBuf::from("/artifacts/note.pdf"),
                staged: Vec::new(),
                invocations: Vec::new(),
                scripted: VecDeque::new(),
            }
        }

        /// Queue the result the next `run` call returns.
        fn script(mut self, result: Result<ToolOutput, RenderError>) -> Self {
            self.scripted.push_back(result);
            self
        }
    }

    impl RenderContext for FakeContext {
        fn stage_file(&mut self, name: &str, contents: &[u8]) -> Result<PathBuf, RenderError> {
            self.staged.push((
                name.to_string(),
                String::from_utf8_lossy(contents).into_owned(),
            ));
            Ok(PathBuf::from(format!("/staging/{name}")))
        }

        fn run(&mut self, invocation: &Invocation) -> Result<ToolOutput, RenderError> {
            self.invocations.push(invocation.clone());
            self.scripted.pop_front().unwrap_or(Ok(ToolOutput {
                success: true,
                stdout: Vec::new(),
                stderr: Vec::new(),
            }))
        }

        fn output_path(&self) -> &Path {
            &self.output
        }
    }

    /// Render the recorded sequence as readable snapshot lines: each staged file
    /// with its contents, then each invocation's program and argv. Argv entries
    /// are lossy strings so unicode and metadata punctuation read plainly.
    fn transcript(ctx: &FakeContext) -> String {
        let mut lines = Vec::new();
        for (name, contents) in &ctx.staged {
            lines.push(format!("STAGE {name}"));
            for line in contents.lines() {
                lines.push(format!("    {line}"));
            }
            if contents.is_empty() {
                lines.push("    <empty>".to_string());
            }
        }
        for invocation in &ctx.invocations {
            lines.push(format!("RUN {}", invocation.program));
            for arg in &invocation.args {
                lines.push(format!("    {}", arg.to_string_lossy()));
            }
        }
        lines.join("\n")
    }

    /// The argv of the single recorded invocation as lossy strings, for tests
    /// that assert on argument presence rather than the whole transcript.
    fn argv(ctx: &FakeContext) -> Vec<String> {
        ctx.invocations[0]
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Build a link spanning `range` in the body. A `Some` title resolves to
    /// emphasized text; `None` leaves a dangling link that keeps its display.
    fn link(range: Range<usize>, display: &str, target_title: Option<&str>) -> ResolvedLink {
        ResolvedLink {
            range,
            display: display.to_string(),
            id: Id::from_str(TARGET_ULID).expect("target ulid parses"),
            target_title: target_title.map(str::to_string),
        }
    }

    /// A prepared document fixture. Tests vary title, tags, body, and links; the
    /// id, path, created date, and frontmatter stay fixed and irrelevant here.
    fn doc(title: &str, tags: &[&str], body: &str, links: Vec<ResolvedLink>) -> PreparedDocument {
        PreparedDocument {
            id: Id::from_str(ULID).expect("ulid parses"),
            path: PathBuf::from("/vault/all-notes/note.md"),
            title: title.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            created: "2020-01-01".to_string(),
            frontmatter: serde_yaml_ng::Mapping::new(),
            body: body.to_string(),
            links,
        }
    }

    #[test]
    fn standard_note_stages_body_and_builds_invocation() {
        // A resolved link becomes emphasized title text; the unresolved one
        // collapses to its display. Two tags produce subtitle and keywords.
        let body = "see [old]({}) and [gone]({}) here".to_string();
        // Compute spans against the literal body.
        let first_start = body.find("[old]").expect("first link present");
        let first_end = first_start + "[old]({})".len();
        let second_start = body.find("[gone]").expect("second link present");
        let second_end = second_start + "[gone]({})".len();
        let links = vec![
            link(first_start..first_end, "old", Some("Current Title")),
            link(second_start..second_end, "gone", None),
        ];
        let document = doc("My Note", &["area/work", "programming/rust"], &body, links);

        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");

        insta::assert_snapshot!(transcript(&ctx), @r"
        STAGE note.md
            see *Current Title* and gone here
        RUN pandoc
            /staging/note.md
            --from
            gfm
            --pdf-engine=typst
            --metadata
            title=My Note
            --metadata
            date=2020-01-01
            --metadata
            subtitle=#area/work · #programming/rust
            --output
            /artifacts/note.pdf
        ");
    }

    #[test]
    fn empty_body_stages_empty_and_still_invokes() {
        let document = doc("Empty", &[], "", Vec::new());
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");

        insta::assert_snapshot!(transcript(&ctx), @r"
        STAGE note.md
            <empty>
        RUN pandoc
            /staging/note.md
            --from
            gfm
            --pdf-engine=typst
            --metadata
            title=Empty
            --metadata
            date=2020-01-01
            --output
            /artifacts/note.pdf
        ");
    }

    #[test]
    fn body_without_links_passes_through_verbatim() {
        let document = doc("Prose", &["a"], "Just prose, no links.\n", Vec::new());
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        assert_eq!(ctx.staged[0].1, "Just prose, no links.\n");
    }

    #[test]
    fn links_at_body_boundaries_are_replaced() {
        // One link occupies the very start of the body, another the very end,
        // exercising the boundary offsets of the back-to-front rewrite.
        let body = "[start]({}) middle [end]({})".to_string();
        let start_end = "[start]({})".len();
        let end_start = body.find("[end]").expect("end link present");
        let end_end = body.len();
        let links = vec![
            link(0..start_end, "start", Some("Alpha")),
            link(end_start..end_end, "end", Some("Omega")),
        ];
        let document = doc("Boundaries", &[], &body, links);
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        assert_eq!(ctx.staged[0].1, "*Alpha* middle *Omega*");
    }

    #[test]
    fn three_links_are_replaced_back_to_front() {
        // The link table is deliberately out of document order to prove the
        // rewrite sorts spans itself rather than trusting their sequence.
        let body = "[one]({}) [two]({}) [three]({})".to_string();
        let one_start = body.find("[one]").expect("one present");
        let one_end = one_start + "[one]({})".len();
        let two_start = body.find("[two]").expect("two present");
        let two_end = two_start + "[two]({})".len();
        let three_start = body.find("[three]").expect("three present");
        let three_end = three_start + "[three]({})".len();
        let links = vec![
            link(two_start..two_end, "two", Some("Two")),
            link(three_start..three_end, "three", None),
            link(one_start..one_end, "one", Some("One")),
        ];
        let document = doc("Three", &[], &body, links);
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        assert_eq!(ctx.staged[0].1, "*One* *Two* three");
    }

    #[test]
    fn unicode_title_and_tags_appear_verbatim_in_argv() {
        let document = doc(
            "Über Größe 日本語",
            &["Área/Work", "Life/Café"],
            "body",
            Vec::new(),
        );
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        let args = argv(&ctx);
        assert!(args.contains(&"title=Über Größe 日本語".to_string()));
        assert!(args.contains(&"subtitle=#Área/Work · #Life/Café".to_string()));
    }

    #[test]
    fn metadata_values_with_special_characters_stay_single_argv_entries() {
        // A title carrying `=` and double quotes, plus a tag carrying `·`, must
        // each ride as one argument: there is no shell to re-split them.
        let document = doc(r#"a=b "quoted""#, &["x·y"], "body", Vec::new());
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        let args = argv(&ctx);
        assert!(args.contains(&r#"title=a=b "quoted""#.to_string()));
        assert!(args.contains(&"subtitle=#x·y".to_string()));
    }

    #[test]
    fn empty_tags_omit_the_subtitle() {
        let document = doc("Untagged", &[], "body", Vec::new());
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        let args = argv(&ctx);
        assert!(!args.iter().any(|a| a.starts_with("subtitle=")));
        insta::assert_snapshot!(args.join("\n"), @r"
        /staging/note.md
        --from
        gfm
        --pdf-engine=typst
        --metadata
        title=Untagged
        --metadata
        date=2020-01-01
        --output
        /artifacts/note.pdf
        ");
    }

    #[test]
    fn two_tags_join_the_subtitle() {
        let document = doc("Tagged", &["a", "b"], "body", Vec::new());
        let mut ctx = FakeContext::new();
        Pandoc.render(&document, &mut ctx).expect("render succeeds");
        let args = argv(&ctx);
        assert!(args.contains(&"subtitle=#a · #b".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("keywords=")));
    }

    #[test]
    fn tool_failure_surfaces_stderr() {
        let document = doc("Fails", &[], "body", Vec::new());
        let mut ctx = FakeContext::new().script(Ok(ToolOutput {
            success: false,
            stdout: Vec::new(),
            stderr: b"typst: page overflow".to_vec(),
        }));
        let err = Pandoc
            .render(&document, &mut ctx)
            .expect_err("a non-zero exit fails the render");
        match err {
            RenderError::ToolFailed { program, stderr } => {
                assert_eq!(program, "pandoc");
                assert_eq!(stderr, "typst: page overflow");
            }
            other => panic!("expected ToolFailed, got {other:?}"),
        }
    }

    #[test]
    fn context_run_error_propagates_unchanged() {
        let document = doc("Unavailable", &[], "body", Vec::new());
        let mut ctx = FakeContext::new().script(Err(RenderError::RendererUnavailable {
            tool: "pandoc".to_string(),
            hint: "install pandoc and typst".to_string(),
        }));
        let err = Pandoc
            .render(&document, &mut ctx)
            .expect_err("a context error stops the render");
        assert!(matches!(err, RenderError::RendererUnavailable { .. }));
    }
}
