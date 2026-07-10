// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The format/engine registry (ADR 0038).
//!
//! A **format** is the artifact kind the user asks for (`pdf`); an **engine** is
//! an implementation that produces one or more formats. `--to` picks the
//! format, `--engine` optionally overrides the engine within it. The registry
//! is the single authority resolving a format-and-optional-engine pair to a
//! concrete [`Renderer`], and it never falls back between engines: an unknown
//! format or engine is an error naming the offending value, so a request for a
//! missing engine cannot be silently served by a different one.

use std::collections::HashMap;

use super::{Pandoc, RenderError, Renderer, Typst};

/// The format selected when the user names none.
pub const DEFAULT_FORMAT: &str = "pdf";

/// Fold an accepted format alias to its canonical name.
///
/// `typ` is an unlisted alias of `typst` (it appears in no help text or docs),
/// so every lookup path normalizes here before touching the format map. An
/// unknown name passes through unchanged, so it still reports as itself.
fn canonical_format(format: &str) -> &str {
    match format {
        "typ" => "typst",
        other => other,
    }
}

/// One format's extension and the engines that produce it.
///
/// The first engine registered for a format becomes its default, so a caller
/// that names no engine resolves to it. Adding an engine is a single
/// [`Registry::register`] call.
struct FormatEntry {
    /// The artifact filename extension (without the dot), e.g. `pdf`.
    extension: &'static str,
    /// The engine used when the caller names none.
    default_engine: String,
    engines: HashMap<String, Box<dyn Renderer>>,
}

/// Maps every known format to the engines that produce it.
pub struct Registry {
    formats: HashMap<String, FormatEntry>,
}

impl Registry {
    /// The registry ntropy ships with: `pdf` produced by the ntropy-owned typst
    /// engine by default, with the pandoc engine still registered and selectable
    /// via `--engine pandoc`; and `typst` produced by the typst engine.
    ///
    /// The typst engine registers for `pdf` before pandoc, so it wins the format
    /// default (the first engine registered for a format becomes its default).
    pub fn new() -> Self {
        let mut registry = Registry {
            formats: HashMap::new(),
        };
        registry.register("pdf", "pdf", "typst", Box::new(Typst::for_pdf_format()));
        registry.register("pdf", "pdf", "pandoc", Box::new(Pandoc));
        registry.register("typst", "typ", "typst", Box::new(Typst::for_typst_format()));
        registry
    }

    /// Register `renderer` as an engine named `engine` producing `format` with
    /// artifact extension `extension`. The first engine registered for a format
    /// becomes its default.
    ///
    /// This is the sole registration path for engines.
    fn register(
        &mut self,
        format: &str,
        extension: &'static str,
        engine: &str,
        renderer: Box<dyn Renderer>,
    ) {
        let entry = self
            .formats
            .entry(format.to_string())
            .or_insert_with(|| FormatEntry {
                extension,
                default_engine: engine.to_string(),
                engines: HashMap::new(),
            });
        entry.engines.insert(engine.to_string(), renderer);
    }

    /// Resolve a format and an optional engine override to the engine that
    /// produces it.
    ///
    /// With no `engine`, the format's default engine is used. An unregistered
    /// format is [`RenderError::UnknownFormat`]; a format that exists but has no
    /// engine of the requested name is [`RenderError::UnknownEngine`], even when
    /// that engine name is registered for a different format.
    pub fn resolve(
        &self,
        format: &str,
        engine: Option<&str>,
    ) -> Result<&dyn Renderer, RenderError> {
        let format = canonical_format(format);
        let entry = self
            .formats
            .get(format)
            .ok_or_else(|| RenderError::UnknownFormat(format.to_string()))?;
        let engine = engine.unwrap_or(&entry.default_engine);
        entry
            .engines
            .get(engine)
            .map(Box::as_ref)
            .ok_or_else(|| RenderError::UnknownEngine {
                format: format.to_string(),
                engine: engine.to_string(),
            })
    }

    /// The artifact filename extension for a format, used to derive the default
    /// output path. An unregistered format is [`RenderError::UnknownFormat`].
    pub fn extension(&self, format: &str) -> Result<&str, RenderError> {
        let format = canonical_format(format);
        self.formats
            .get(format)
            .map(|entry| entry.extension)
            .ok_or_else(|| RenderError::UnknownFormat(format.to_string()))
    }

    /// The name of the engine a format resolves to when the caller names none,
    /// so a report can say which engine produced an artifact. An unregistered
    /// format is [`RenderError::UnknownFormat`].
    pub fn default_engine(&self, format: &str) -> Result<&str, RenderError> {
        let format = canonical_format(format);
        self.formats
            .get(format)
            .map(|entry| entry.default_engine.as_str())
            .ok_or_else(|| RenderError::UnknownFormat(format.to_string()))
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PreparedDocument, RenderContext};

    /// A no-op engine standing in for a real one, so registry resolution and
    /// extension lookup are exercised without any external tool.
    struct DummyRenderer;

    impl Renderer for DummyRenderer {
        fn render(
            &self,
            _doc: &PreparedDocument,
            _ctx: &mut dyn RenderContext,
        ) -> Result<(), RenderError> {
            Ok(())
        }
    }

    /// A registry with two formats, each served by one distinctly named engine.
    /// The second format lets a test prove an engine known for another format is
    /// still `UnknownEngine`.
    fn populated() -> Registry {
        let mut registry = Registry::new();
        registry.register("pdf", "pdf", "dummy", Box::new(DummyRenderer));
        registry.register("html", "html", "web", Box::new(DummyRenderer));
        registry
    }

    #[test]
    fn resolve_default_engine() {
        let registry = populated();
        assert!(registry.resolve("pdf", None).is_ok());
    }

    #[test]
    fn resolve_explicit_engine() {
        let registry = populated();
        assert!(registry.resolve("pdf", Some("dummy")).is_ok());
    }

    #[test]
    fn resolve_unknown_format() {
        let registry = populated();
        // `&dyn Renderer` is not `Debug`, so unwrap the error side directly.
        let err = registry
            .resolve("docx", None)
            .err()
            .expect("an unregistered format does not resolve");
        insta::assert_snapshot!(err, @"unknown output format `docx`");
    }

    #[test]
    fn resolve_unknown_engine() {
        let registry = populated();
        let err = registry
            .resolve("pdf", Some("wkhtml"))
            .err()
            .expect("an unregistered engine does not resolve");
        insta::assert_snapshot!(err, @"unknown engine `wkhtml` for format `pdf`");
    }

    #[test]
    fn engine_of_another_format_is_unknown_here() {
        // `web` produces `html`, so asking for it under `pdf` is UnknownEngine:
        // the registry never crosses format boundaries to satisfy an override.
        let registry = populated();
        let err = registry
            .resolve("pdf", Some("web"))
            .err()
            .expect("a foreign-format engine does not resolve");
        insta::assert_snapshot!(err, @"unknown engine `web` for format `pdf`");
    }

    #[test]
    fn extension_known() {
        let registry = populated();
        assert_eq!(registry.extension("pdf").expect("pdf is registered"), "pdf");
        assert_eq!(
            registry.extension("html").expect("html is registered"),
            "html"
        );
    }

    #[test]
    fn default_engine_names_the_first_registered() {
        // The shipping registry registers the typst engine for `pdf` first, so
        // it stays the default even after `populated` adds another `pdf` engine.
        let registry = populated();
        assert_eq!(
            registry
                .default_engine("pdf")
                .expect("pdf has a default engine"),
            "typst"
        );
        let err = registry
            .default_engine("docx")
            .expect_err("an unregistered format has no default engine");
        insta::assert_snapshot!(err, @"unknown output format `docx`");
    }

    #[test]
    fn extension_unknown() {
        let registry = populated();
        let err = registry
            .extension("docx")
            .expect_err("an unregistered format has no extension");
        insta::assert_snapshot!(err, @"unknown output format `docx`");
    }

    /// The shipping registry serves `pdf` through the typst engine by default:
    /// the default format resolves with no engine named to `typst`, the pandoc
    /// engine stays selectable via an explicit override, `typst` names it
    /// explicitly too, and its extension is `pdf`. An unknown engine for `pdf`
    /// is still an error.
    #[test]
    fn shipped_registry_resolves_pdf_through_typst_by_default() {
        let registry = Registry::new();
        assert_eq!(
            registry
                .default_engine(DEFAULT_FORMAT)
                .expect("pdf has a default engine"),
            "typst"
        );
        assert!(registry.resolve(DEFAULT_FORMAT, None).is_ok());
        assert!(registry.resolve(DEFAULT_FORMAT, Some("typst")).is_ok());
        assert!(registry.resolve(DEFAULT_FORMAT, Some("pandoc")).is_ok());
        assert_eq!(
            registry
                .extension(DEFAULT_FORMAT)
                .expect("pdf is registered"),
            "pdf"
        );
        assert!(registry.resolve(DEFAULT_FORMAT, Some("wkhtml")).is_err());
    }

    /// The shipping registry serves `typst` through the typst engine: the format
    /// resolves with no engine named and with `typst` named explicitly, and its
    /// extension is `typ`. Both `typst` and `pdf` default to the typst engine,
    /// which serves each with its own format-specific delivery.
    #[test]
    fn shipped_registry_resolves_typst_through_the_typst_engine() {
        let registry = Registry::new();
        assert!(registry.resolve("typst", None).is_ok());
        assert!(registry.resolve("typst", Some("typst")).is_ok());
        assert_eq!(
            registry.extension("typst").expect("typst is registered"),
            "typ"
        );
        assert_eq!(
            registry
                .default_engine("typst")
                .expect("typst has a default engine"),
            "typst"
        );
        assert_eq!(
            registry.default_engine("pdf").expect("pdf is registered"),
            "typst"
        );
    }

    /// The pandoc engine produces `pdf`, not `typst`, so requesting it under
    /// `typst` is `UnknownEngine`: the registry never crosses format boundaries.
    #[test]
    fn typst_format_rejects_the_pandoc_engine() {
        let registry = Registry::new();
        let err = registry
            .resolve("typst", Some("pandoc"))
            .err()
            .expect("a foreign-format engine does not resolve");
        insta::assert_snapshot!(err, @"unknown engine `pandoc` for format `typst`");
    }

    /// `typ` is an unlisted alias of `typst`: it resolves the same engine, the
    /// same extension, and the same default across every lookup path.
    #[test]
    fn typ_alias_resolves_like_typst() {
        let registry = Registry::new();
        assert!(registry.resolve("typ", None).is_ok());
        assert_eq!(
            registry.extension("typ").expect("the alias resolves"),
            "typ"
        );
        assert_eq!(
            registry
                .default_engine("typ")
                .expect("the alias has a default engine"),
            "typst"
        );
        // The alias only reaches the canonical format, never a foreign engine.
        assert!(registry.resolve("typ", Some("pandoc")).is_err());
    }
}
