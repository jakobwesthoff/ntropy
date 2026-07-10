// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The embedded default prelude: the engine-owned `note` and `callout`
//! definitions inlined ahead of every emitted document
//! (`docs/design/typst-engine.md`, "Document skeleton").
//!
//! The prelude is the single seam between content and presentation. Document
//! assembly inlines it verbatim so the emitted `typst` artifact is a
//! self-contained file that compiles on its own, and so a theme can replace
//! the definitions without any change to the converted body. The source lives
//! in `prelude.typ` next to this module and is embedded at compile time,
//! keeping it editable and syntax-highlightable as Typst rather than trapped
//! in a Rust string.

/// The default prelude source, embedded from `prelude.typ`.
///
/// Defines `callout` (a bordered block with a bold, capitalized kind lead-in,
/// all five GFM kinds and any unknown kind rendered through the one form) and
/// `note` (prominent centered title, every frontmatter field as a key-value
/// line, then the body).
pub const PRELUDE: &str = include_str!("prelude.typ");

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded prelude must parse without errors through the same Typst
    /// parser the rest of the engine is verified against. A malformed prelude
    /// would break every emitted document, so this is the cheapest guard that
    /// keeps the embedded source valid.
    #[test]
    fn prelude_parses_without_errors() {
        let root = typst_syntax::parse(PRELUDE);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(errors.is_empty(), "prelude parse errors: {errors:?}");
    }

    /// The definitions the document skeleton and the emitter depend on must be
    /// present. A string check is enough: assembly and the emitter reference
    /// these names by hand, so their absence is a contract break regardless of
    /// the prelude's internals.
    #[test]
    fn prelude_defines_the_engine_facing_functions() {
        assert!(PRELUDE.contains("#let callout("));
        assert!(PRELUDE.contains("#let note("));
        assert!(PRELUDE.contains("#let notelink("));
        assert!(PRELUDE.contains("#let task("));
    }
}
