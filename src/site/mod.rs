// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The website layer: what turns converted HTML into pages
//! (`docs/design/site-export.md`).
//!
//! - [`options`]: the `[site]` table of the vault config.
//! - [`theme`]: site themes, stylesheets and assets in
//!   `<vault>/.ntropy/themes/site/<name>/`, and the embedded default.
//! - [`page`]: the minijinja templates embedded in the binary and the typed
//!   contexts they render from.
//! - [`model`]: the site's structure, computed from the notes and views.
//! - [`nav`]: the navigation fragments built from the model.
//! - [`build`]: every file of a site, built in memory, and their writing.

pub mod build;
pub mod model;
pub mod nav;
pub mod options;
pub mod page;
pub mod theme;

pub use build::{Built, Input, build, write};

pub use options::SiteOptions;
pub use theme::SiteTheme;

/// What the `html` render format needs from the site layer: the theme whose
/// stylesheet a document inlines, and the page language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSettings {
    /// The selected site theme, or `None` for the built-in one.
    pub theme: Option<SiteTheme>,
    /// The `lang` attribute of the page.
    pub lang: String,
}

impl Default for DocumentSettings {
    fn default() -> Self {
        DocumentSettings {
            theme: None,
            lang: options::DEFAULT_LANG.to_string(),
        }
    }
}
