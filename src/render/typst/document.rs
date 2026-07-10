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

use serde_yaml_ng::{Mapping, Value};

use super::prelude::PRELUDE;
use super::value::value_literal;
use super::writer::TypstWriter;

/// Assemble the complete emitted document from a note's title, frontmatter, and
/// already-converted body.
///
/// The layout is fixed: the embedded prelude, a blank line, the template
/// application, a blank line, then the body spliced in verbatim (it is already
/// Typst markup produced by the emitter). The title is emitted through the
/// writer's string-literal channel, and the frontmatter through
/// [`value_literal`] on the whole mapping, so both reach the template as inert
/// data.
// The typst engine of this same phase is the production consumer; until it
// wires assembly into the render pipeline the crate has no in-crate caller,
// and clippy runs with `-D warnings`. The reviewer removes this allow when the
// engine calls `assemble`.
#[allow(dead_code)]
pub fn assemble(title: &str, frontmatter: &Mapping, body: &str) -> String {
    let mut writer = TypstWriter::new();
    writer.syntax(PRELUDE);

    // The template application. The trailing comma after `frontmatter:` keeps
    // the argument list well-formed however the literal ends.
    writer.syntax("\n#show: note.with(title: \"");
    writer.string_literal(title);
    writer.syntax("\", frontmatter: ");
    writer.syntax(&value_literal(&Value::Mapping(frontmatter.clone())));
    writer.syntax(",)\n\n");

    writer.raw(body);
    writer.finish()
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
        let document = assemble("Example Note", &fm, body);
        insta::assert_snapshot!(document);
        assert_parses(&document);
    }

    #[test]
    fn title_with_quotes_and_backslashes_is_escaped() {
        let document = assemble(r#"a "quoted" \ title"#, &Mapping::new(), "body");
        assert!(
            document.contains(r#"note.with(title: "a \"quoted\" \\ title""#),
            "title not string-literal escaped: {document}"
        );
        assert_parses(&document);
    }

    #[test]
    fn empty_frontmatter_is_the_colon_dictionary() {
        let document = assemble("Title", &Mapping::new(), "body");
        assert!(
            document.contains("frontmatter: (:),)"),
            "empty frontmatter not emitted as `(:)`: {document}"
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
        let document = assemble("Title", &fm, "body");
        assert!(
            document.contains(r#"frontmatter: ("outer": ("inner": (1, 2)))"#),
            "nested frontmatter not translated: {document}"
        );
        assert_parses(&document);
    }

    #[test]
    fn body_is_spliced_verbatim() {
        // The body is already emitter output; assembly must not touch it.
        let body = "#emph[unchanged] and a `raw` run with \\backslashes\n";
        let document = assemble("Title", &Mapping::new(), body);
        assert!(
            document.ends_with(body),
            "body was not spliced verbatim at the end: {document}"
        );
    }
}
