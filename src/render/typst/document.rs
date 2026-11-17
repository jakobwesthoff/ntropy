// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Document assembly: the prelude, the template application, and the converted
//! body joined into one emitted file (`docs/design/typst-engine.md`,
//! "Document skeleton").
//!
//! One assembled file serves both output formats: the `typst` format hands it
//! out as the artifact, and the `pdf` format compiles that identical file.
//! `#show: note.with(title: ..., frontmatter: ...)` is the single seam between
//! content and presentation. The title crosses that seam as a string literal
//! and the frontmatter as a typed Typst value, so neither can ever be read as
//! Typst code whatever it contains.
//!
//! A theme's source is spliced between the prelude and that line (ADR 0045).
//! Typst binds a name to the last `#let` that precedes its use, so a theme
//! redefining `note`, `callout`, `notelink` or `task` shadows the prelude's
//! version for everything below, while a theme that defines only some of them
//! inherits the rest.

use serde_yaml_ng::{Mapping, Value};

use super::prelude::PRELUDE;
use super::value::value_literal;
use super::writer::TypstWriter;
use crate::note::frontmatter::{scalar_text, untag};
use crate::render::Paper;

/// Assemble the complete emitted document from a note's title, frontmatter,
/// paper format, and already-converted body.
///
/// The layout is fixed: the embedded prelude, a blank line, the template
/// application, a blank line, then the body spliced in verbatim (it is already
/// Typst markup produced by the emitter). The title is emitted through the
/// writer's string-literal channel, the frontmatter through [`value_literal`]
/// on the whole mapping, and the paper as its canonical name — the paper names
/// are Typst's own identifiers, which the template's `set page` accepts
/// directly.
pub fn assemble(
    title: &str,
    frontmatter: &Mapping,
    paper: Paper,
    theme: Option<&str>,
    body: &str,
) -> String {
    let mut writer = TypstWriter::new();
    writer.syntax(PRELUDE);

    // The theme follows the prelude verbatim and unescaped: it is Typst source
    // the vault owner wrote, exactly like the prelude beside it. A marker
    // comment keeps the seam legible in an emitted `typst` artifact, which is
    // a file a user reads and compiles by hand.
    if let Some(source) = theme {
        writer.syntax("\n// ---- vault theme ----\n");
        writer.syntax(source);
        if !source.ends_with('\n') {
            writer.syntax("\n");
        }
    }

    // The template application. The trailing comma after the last argument
    // keeps the list well-formed however the literal ends.
    writer.syntax("\n#show: note.with(title: \"");
    writer.string_literal(title);
    writer.syntax("\", frontmatter: ");
    writer.syntax(&value_literal(&Value::Mapping(readable(frontmatter))));
    writer.syntax(", paper: \"");
    writer.syntax(paper.as_str());
    writer.syntax("\",)\n\n");

    writer.raw(body);
    writer.finish()
}

/// The frontmatter as the theme receives it: `title` and `tags` read as the
/// text ADR 0081 makes of them, every other field a typed value.
///
/// A theme draws a tag as text, and Typst's `text` rejects an integer, a float
/// or a boolean. Reading the two recognized fields the way the note loader
/// does keeps `tags: [2026]` a list of strings here, and a null entry is
/// dropped as the loader skips it, rather than drawn as an empty chip.
fn readable(frontmatter: &Mapping) -> Mapping {
    let mut readable = frontmatter.clone();

    if let Some(title) = readable.get_mut("title")
        && let Some(text) = scalar_text(title)
    {
        *title = Value::String(text);
    }

    if let Some(tags) = readable.get_mut("tags") {
        match untag(tags) {
            Value::Sequence(entries) => {
                let texts = entries
                    .iter()
                    .filter(|entry| !matches!(untag(entry), Value::Null))
                    .map(|entry| scalar_text(entry).map_or_else(|| entry.clone(), Value::String))
                    .collect();
                *tags = Value::Sequence(texts);
            }
            lone => {
                if let Some(text) = scalar_text(lone) {
                    *tags = Value::String(text);
                }
            }
        }
    }

    readable
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse an assembled document and require it error-free through the same
    /// Typst parser the rest of the engine is verified against. A malformed
    /// template application or a body that breaks the surrounding structure
    /// surfaces here.
    fn assert_parses(document: &str) {
        let root = typst_syntax::parse(document);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(
            errors.is_empty(),
            "assembled document parse errors: {errors:?}\n---\n{document}"
        );
    }

    /// Build a frontmatter mapping from inline YAML.
    fn frontmatter(yaml: &str) -> Mapping {
        serde_yaml_ng::from_str(yaml).expect("the test YAML parses into a mapping")
    }

    #[test]
    fn representative_document_snapshot() {
        let fm = frontmatter(
            r#"
            title: Example Note
            tags:
              - area/work
              - programming/rust
            created: "2026-07-10"
            meta:
              draft: true
              revision: 3
            "#,
        );
        let body = "= A heading\n\nSome body text.\n";
        let document = assemble("Example Note", &fm, Paper::A4, None, body);
        insta::assert_snapshot!(document);
        assert_parses(&document);
    }

    #[test]
    fn title_with_quotes_and_backslashes_is_escaped() {
        let document = assemble(
            r#"a "quoted" \ title"#,
            &Mapping::new(),
            Paper::A4,
            None,
            "body",
        );
        assert!(
            document.contains(r#"note.with(title: "a \"quoted\" \\ title""#),
            "title not string-literal escaped: {document}"
        );
        assert_parses(&document);
    }

    #[test]
    fn empty_frontmatter_is_the_colon_dictionary() {
        let document = assemble("Title", &Mapping::new(), Paper::A4, None, "body");
        assert!(
            document.contains("frontmatter: (:),"),
            "empty frontmatter not emitted as `(:)`: {document}"
        );
        assert_parses(&document);
    }

    #[test]
    fn the_paper_name_reaches_the_template() {
        let document = assemble("Title", &Mapping::new(), Paper::UsLetter, None, "body");
        assert!(
            document.contains(r#"paper: "us-letter",)"#),
            "configured paper not applied: {document}"
        );
        assert_parses(&document);
    }

    #[test]
    fn nested_frontmatter_translates_and_parses() {
        let fm = frontmatter(
            r#"
            outer:
              inner:
                - 1
                - 2
            "#,
        );
        let document = assemble("Title", &fm, Paper::A4, None, "body");
        assert!(
            document.contains(r#"frontmatter: ("outer": ("inner": (1, 2)))"#),
            "nested frontmatter not translated: {document}"
        );
        assert_parses(&document);
    }

    // =====================================================================
    // Themes
    // =====================================================================

    #[test]
    fn a_theme_is_spliced_between_the_prelude_and_the_template_application() {
        // Order is the whole mechanism: Typst binds `note` to the last `#let`
        // above its use, so the theme must land after the prelude and before
        // the `#show` line for its definition to be the one that runs.
        let theme = "#let note(title: none, frontmatter: (:), paper: \"a4\", body) = body\n";
        let document = assemble("Title", &Mapping::new(), Paper::A4, Some(theme), "body");

        let theme_at = document.find(theme).expect("the theme is in the document");
        let prelude_at = document
            .find("#let notelink")
            .expect("the prelude is in the document");
        let show_at = document
            .find("#show: note.with")
            .expect("the template application is in the document");
        assert!(
            prelude_at < theme_at && theme_at < show_at,
            "theme is not between the prelude and the template application"
        );
        assert_parses(&document);
    }

    #[test]
    fn a_theme_is_spliced_verbatim() {
        // A theme is Typst source the vault owner wrote; assembly must not
        // escape or reformat it, or a `#let` would stop being one.
        let theme = "#let note(body) = body\n#let x = \"a \\\" quote\"\n";
        let document = assemble("Title", &Mapping::new(), Paper::A4, Some(theme), "body");
        assert!(
            document.contains(theme),
            "theme was not spliced verbatim: {document}"
        );
    }

    #[test]
    fn a_theme_without_a_trailing_newline_still_separates_from_what_follows() {
        // Concatenating a theme that ends mid-line onto the `#show` line would
        // fuse the two into one broken statement.
        let theme = "#let note(title: none, frontmatter: (:), paper: \"a4\", body) = body";
        let document = assemble("Title", &Mapping::new(), Paper::A4, Some(theme), "body");
        assert!(
            document.contains("= body\n"),
            "no newline after the theme: {document}"
        );
        assert_parses(&document);
    }

    #[test]
    fn no_theme_leaves_the_document_exactly_as_before() {
        // The built-in look is the same bytes it always was, so an unthemed
        // vault is untouched by this feature.
        let document = assemble("Title", &Mapping::new(), Paper::A4, None, "body");
        assert!(
            !document.contains("vault theme"),
            "an unthemed document carries a theme marker: {document}"
        );
    }

    /// The frontmatter literal an assembled document carries for `yaml`.
    fn emitted_frontmatter(yaml: &str) -> String {
        let document = assemble("Title", &frontmatter(yaml), Paper::A4, None, "body");
        assert_parses(&document);
        // The prelude defines `note` with a `frontmatter: (:)` default of its
        // own, so the search starts at the template application.
        let application = document
            .find("#show: note.with(")
            .expect("the document applies the template");
        let start = application
            + document[application..]
                .find("frontmatter: ")
                .expect("the template application carries the frontmatter")
            + "frontmatter: ".len();
        let end = document[start..]
            .find(", paper:")
            .expect("the paper follows the frontmatter");
        document[start..start + end].to_string()
    }

    #[test]
    fn an_integer_tag_is_emitted_as_a_string() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: [2026]\n"),
            r#"("title": "T", "tags": ("2026",))"#
        );
    }

    #[test]
    fn a_boolean_tag_is_emitted_as_a_string() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: [true]\n"),
            r#"("title": "T", "tags": ("true",))"#
        );
    }

    #[test]
    fn a_float_tag_is_emitted_as_its_parsed_spelling() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: [1.50]\n"),
            r#"("title": "T", "tags": ("1.5",))"#
        );
    }

    #[test]
    fn a_numeric_title_in_the_frontmatter_is_emitted_as_a_string() {
        assert_eq!(emitted_frontmatter("title: 2026\n"), r#"("title": "2026")"#);
    }

    #[test]
    fn a_lone_scalar_tags_field_is_emitted_as_a_string() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: 2026\n"),
            r#"("title": "T", "tags": "2026")"#
        );
    }

    #[test]
    fn a_tagged_scalar_tag_is_emitted_as_its_text() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: [!label 2026]\n"),
            r#"("title": "T", "tags": ("2026",))"#
        );
    }

    #[test]
    fn a_tagged_tags_list_is_emitted_as_an_array_of_strings() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: !label [2026, a]\n"),
            r#"("title": "T", "tags": ("2026", "a"))"#
        );
    }

    #[test]
    fn a_null_tag_entry_is_dropped() {
        assert_eq!(
            emitted_frontmatter("title: T\ntags: [a, ~, b]\n"),
            r#"("title": "T", "tags": ("a", "b"))"#
        );
    }

    #[test]
    fn other_fields_keep_their_typed_values() {
        assert_eq!(
            emitted_frontmatter("title: T\npriority: 3\ndraft: true\nratio: 1.5\n"),
            r#"("title": "T", "priority": 3, "draft": true, "ratio": 1.5)"#
        );
    }

    #[test]
    fn body_is_spliced_verbatim() {
        // The body is already emitter output; assembly must not touch it.
        let body = "#emph[unchanged] and a `raw` run with \\backslashes\n";
        let document = assemble("Title", &Mapping::new(), Paper::A4, None, body);
        assert!(
            document.ends_with(body),
            "body was not spliced verbatim at the end: {document}"
        );
    }
}
