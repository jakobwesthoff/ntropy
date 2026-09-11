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
//! - [`frontend`]: the embedded page script and highlighting grammars.
//! - [`search`]: the search data every site embeds for the page script.

pub mod build;
pub mod frontend;
pub mod model;
pub mod nav;
pub mod options;
pub mod page;
pub mod search;
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// Every frontend source carries the MPL-2.0 header in its comment
    /// syntax; the built output under `src/site/dist/` carries none, like
    /// the vault seed content (ADR 0051). JSON files cannot hold a comment
    /// and are exempt.
    #[test]
    fn frontend_sources_carry_the_license_header_and_built_output_does_not() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let site = root.join("site");
        let mut sources: Vec<PathBuf> = vec![site.join("build.ts"), site.join("vite.config.ts")];
        collect(&site.join("src"), &mut sources);
        assert!(sources.len() > 2, "the frontend sources were found");
        for path in &sources {
            let text = std::fs::read_to_string(path).expect("source is readable");
            assert!(
                text.starts_with(
                    "// This Source Code Form is subject to the terms of the Mozilla Public\n"
                ),
                "{} lacks the MPL-2.0 header",
                path.display()
            );
        }
        let app = std::fs::read_to_string(root.join("src/site/dist/app.js")).expect("dist");
        assert!(
            !app.contains("Mozilla Public"),
            "the built page script is generated and carries no header"
        );
    }

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("dir is readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                collect(&path, out);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ts" | "tsx" | "css")
            ) {
                out.push(path);
            }
        }
    }
}
