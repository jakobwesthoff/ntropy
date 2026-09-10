// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The engine behind the `html` format of `render` (ADR 0046).
//!
//! One self-contained file per note: the site theme's stylesheet inlined, the
//! note's header and body, and note links targeting `<slug>.html` beside the
//! artifact, the HTML counterpart of the `<slug>.pdf` convention (ADR 0044).
//! No sidebar, no search; the site export builds those around the same
//! converted body.

use super::emitter::{self, SiblingArtifacts};
use super::frontmatter;
use crate::render::{PreparedDocument, RenderContext, RenderError, Renderer};
use crate::site::page::Document;
use crate::site::{DocumentSettings, SiteTheme};

/// The ntropy-owned engine registered as `html`.
pub struct Html {
    settings: DocumentSettings,
}

impl Html {
    pub fn new(settings: DocumentSettings) -> Self {
        Html { settings }
    }
}

impl Renderer for Html {
    fn render(
        &self,
        doc: &PreparedDocument,
        ctx: &mut dyn RenderContext,
    ) -> Result<(), RenderError> {
        // Asset paths stay as written: the artifact is one file wherever
        // `-o` put it, and a path relative to the note is what the author
        // meant. Note links target the sibling artifact by default name.
        let emitted = emitter::emit(&doc.body, &doc.links, &SiblingArtifacts);
        for warning in &emitted.warnings {
            ctx.warn(&warning.message);
        }

        let page = Document {
            lang: self.settings.lang.clone(),
            title: doc.title.clone(),
            created: doc.created.clone(),
            tags: doc.tags.clone(),
            fields: frontmatter::fields(&doc.frontmatter),
            stylesheet: SiteTheme::stylesheet_of(self.settings.theme.as_ref()),
            body: emitted.html,
        };
        ctx.write_output(page.render().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Id;
    use crate::render::{Invocation, LinkTarget, ResolvedLink, ToolOutput};

    use std::ops::Range;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// A [`RenderContext`] that records the written artifact and the warnings.
    /// The html engine runs no tool and stages nothing; both panic if reached.
    struct FakeContext {
        output: PathBuf,
        warnings: Vec<String>,
        written: Option<Vec<u8>>,
    }

    impl FakeContext {
        fn new() -> Self {
            FakeContext {
                output: PathBuf::from("/artifacts/note.html"),
                warnings: Vec::new(),
                written: None,
            }
        }

        fn page(&self) -> String {
            String::from_utf8(self.written.clone().expect("the engine wrote an artifact"))
                .expect("the artifact is valid UTF-8")
        }
    }

    impl RenderContext for FakeContext {
        fn stage_file(&mut self, _name: &str, _contents: &[u8]) -> Result<PathBuf, RenderError> {
            panic!("the html engine stages nothing");
        }

        fn run(&mut self, _invocation: &Invocation) -> Result<ToolOutput, RenderError> {
            panic!("the html engine runs no external tool");
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
            tags: vec!["area/work".to_string()],
            created: "2020-01-01".to_string(),
            frontmatter: serde_yaml_ng::from_str(frontmatter)
                .expect("the fixture frontmatter parses into a mapping"),
            body: body.to_string(),
            links,
        }
    }

    fn resolved_link(range: Range<usize>, title: &str, slug: &str) -> ResolvedLink {
        ResolvedLink {
            range,
            display: String::new(),
            id: Id::from_str(ULID).expect("ulid parses"),
            target: Some(LinkTarget {
                title: title.to_string(),
                slug: slug.to_string(),
            }),
        }
    }

    #[test]
    fn writes_a_self_contained_page_with_the_builtin_stylesheet() {
        let document = doc(
            "My Note",
            "priority: 2\n",
            "# Heading\n\nSome *body* text.\n",
            Vec::new(),
        );
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");

        let page = ctx.page();
        assert!(page.starts_with("<!doctype html>"), "{page}");
        assert!(page.contains("<html lang=\"en\">"), "{page}");
        assert!(page.contains("<title>My Note</title>"), "{page}");
        assert!(
            page.contains(&SiteTheme::builtin().stylesheet),
            "the built-in stylesheet is inlined"
        );
        assert!(page.contains("<li class=\"tag\">area/work</li>"), "{page}");
        assert!(page.contains("<dt>priority</dt>\n<dd>2</dd>"), "{page}");
        assert!(page.contains("<h1 id=\"heading\">Heading</h1>"), "{page}");
        assert!(page.contains("<em>body</em>"), "{page}");
        assert!(ctx.warnings.is_empty());
    }

    #[test]
    fn a_vault_theme_and_language_replace_the_defaults() {
        let settings = DocumentSettings {
            theme: Some(SiteTheme {
                name: "corporate".to_string(),
                stylesheet: "body { color: rebeccapurple }".to_string(),
            }),
            lang: "de".to_string(),
        };
        let document = doc("Themed", "{}", "text\n", Vec::new());
        let mut ctx = FakeContext::new();
        Html::new(settings)
            .render(&document, &mut ctx)
            .expect("render succeeds");
        let page = ctx.page();
        assert!(page.contains("<html lang=\"de\">"), "{page}");
        assert!(page.contains("body { color: rebeccapurple }"), "{page}");
        assert!(
            !page.contains("--callout-note"),
            "the built-in stylesheet is absent"
        );
    }

    #[test]
    fn a_note_link_targets_the_sibling_artifact() {
        let body = "See [old](01BX5ZZKBKACTAV9WEVGEMMVRZ-old.md).\n";
        let range = 4..body.len() - 2;
        let document = doc(
            "Linking",
            "{}",
            body,
            vec![resolved_link(range, "Current Title", "current-title")],
        );
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");
        assert!(
            ctx.page()
                .contains("<a class=\"note-link\" href=\"current-title.html\">Current Title</a>"),
            "{}",
            ctx.page()
        );
    }

    #[test]
    fn emitter_warnings_reach_the_host() {
        // A link to a file outside the vault is the html emitter's one
        // warning-free degradation today; raw HTML passes through and remote
        // images stay remote, so the html format warns for nothing the
        // kitchen sink contains. This pins that the forwarding path exists
        // by checking a clean note forwards nothing, and the emitter's own
        // tests pin what it warns about.
        let document = doc(
            "Clean",
            "{}",
            "<div>raw</div>\n\n![r](https://e.org/i.png)\n",
            Vec::new(),
        );
        let mut ctx = FakeContext::new();
        Html::new(DocumentSettings::default())
            .render(&document, &mut ctx)
            .expect("render succeeds");
        assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
        assert!(ctx.page().contains("<div>raw</div>"));
    }
}
