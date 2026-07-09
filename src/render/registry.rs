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

use super::{Pandoc, RenderError, Renderer};

/// The format selected when the user names none.
pub const DEFAULT_FORMAT: &str = "pdf";

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
    /// The registry ntropy ships with: `pdf` produced by the pandoc engine.
    pub fn new() -> Self {
        let mut registry = Registry {
            formats: HashMap::new(),
        };
        registry.register("pdf", "pdf", "pandoc", Box::new(Pandoc));
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
        self.formats
            .get(format)
            .map(|entry| entry.extension)
            .ok_or_else(|| RenderError::UnknownFormat(format.to_string()))
    }

    /// The name of the engine a format resolves to when the caller names none,
    /// so a report can say which engine produced an artifact. An unregistered
    /// format is [`RenderError::UnknownFormat`].
    pub fn default_engine(&self, format: &str) -> Result<&str, RenderError> {
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
        let registry = populated();
        assert_eq!(
            registry
                .default_engine("pdf")
                .expect("pdf has a default engine"),
            "pandoc"
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

    /// The shipping registry serves `pdf` through pandoc: the default format
    /// resolves with no engine named and with `pandoc` named explicitly, and its
    /// extension is `pdf`. An unknown engine for `pdf` is still an error.
    #[test]
    fn shipped_registry_resolves_pdf_through_pandoc() {
        let registry = Registry::new();
        assert!(registry.resolve(DEFAULT_FORMAT, None).is_ok());
        assert!(registry.resolve(DEFAULT_FORMAT, Some("pandoc")).is_ok());
        assert_eq!(
            registry
                .extension(DEFAULT_FORMAT)
                .expect("pdf is registered"),
            "pdf"
        );
        assert!(registry.resolve(DEFAULT_FORMAT, Some("wkhtml")).is_err());
    }
}
