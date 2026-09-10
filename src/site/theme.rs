// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Site themes: the built-in one and those a vault holds (ADR 0048).
//!
//! A site theme is a directory `<vault>/.ntropy/themes/site/<name>/` of
//! stylesheets and static assets over an HTML structure that is ntropy's own.
//! Its entry point is `style.css`; every file in the directory is shipped
//! beside the pages. The built-in theme is the same set of files embedded in
//! the binary, and `site theme init` writes them into a vault as the starting
//! point for a custom theme.
//!
//! Selection reuses the render themes' rule: `--theme` over `[site] theme`,
//! with the reserved name `default` meaning the built-in theme from either
//! source ([`crate::render::theme::select`]).

use std::path::Path;

use crate::render::RenderError;
use crate::render::theme::validate_name;
use crate::vault::layout::Layout;

/// The stylesheet a site theme directory must contain.
pub const STYLESHEET: &str = "style.css";

/// The files of the built-in theme, as `(relative path, contents)`. The
/// stylesheet is first.
pub const BUILTIN_FILES: &[(&str, &[u8])] = &[(STYLESHEET, include_bytes!("theme/style.css"))];

/// A site theme ready to use: its name and its stylesheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteTheme {
    /// The name as selected, for reports; `default` for the built-in theme.
    pub name: String,
    /// The contents of `style.css`.
    pub stylesheet: String,
}

impl SiteTheme {
    /// The theme embedded in the binary.
    pub fn builtin() -> Self {
        let (_, css) = BUILTIN_FILES[0];
        SiteTheme {
            name: crate::render::theme::DEFAULT_THEME.to_string(),
            stylesheet: String::from_utf8(css.to_vec())
                .expect("the embedded stylesheet is UTF-8 by construction"),
        }
    }

    /// The stylesheet of `theme`, or of the built-in theme when none is
    /// selected.
    pub fn stylesheet_of(theme: Option<&SiteTheme>) -> String {
        match theme {
            Some(theme) => theme.stylesheet.clone(),
            None => SiteTheme::builtin().stylesheet,
        }
    }
}

/// Load the site theme named `name` from the vault.
///
/// The name is validated as a single path component first. A theme
/// directory without a `style.css` is reported as a missing theme naming
/// that path, since the stylesheet is what makes a directory a theme.
pub fn load(layout: &Layout, name: &str) -> Result<SiteTheme, RenderError> {
    validate_name(name)?;
    let path = stylesheet_path(layout, name);
    match std::fs::read_to_string(&path) {
        Ok(stylesheet) => Ok(SiteTheme {
            name: name.to_string(),
            stylesheet,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(RenderError::ThemeNotFound {
            name: name.to_string(),
            path,
        }),
        Err(source) => Err(RenderError::ThemeRead { path, source }),
    }
}

/// `<vault>/.ntropy/themes/site/<name>/style.css`.
pub fn stylesheet_path(layout: &Layout, name: &str) -> std::path::PathBuf {
    layout.site_theme_dir(name).join(STYLESHEET)
}

/// Write the built-in theme's files into `dir`, which must not exist yet.
pub fn write_builtin(dir: &Path) -> std::io::Result<()> {
    if dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", dir.display()),
        ));
    }
    std::fs::create_dir_all(dir)?;
    for (relative, contents) in BUILTIN_FILES {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempfile::TempDir, Layout) {
        let dir = tempfile::tempdir().expect("temp dir");
        let layout = Layout::new(dir.path());
        std::fs::create_dir_all(layout.site_themes_dir()).expect("themes dir");
        (dir, layout)
    }

    #[test]
    fn the_builtin_theme_has_a_stylesheet() {
        let theme = SiteTheme::builtin();
        assert_eq!(theme.name, "default");
        assert!(
            theme.stylesheet.contains("--"),
            "the stylesheet defines variables"
        );
        assert_eq!(BUILTIN_FILES[0].0, STYLESHEET);
    }

    #[test]
    fn a_vault_theme_loads_its_stylesheet() {
        let (_dir, layout) = vault();
        let dir = layout.site_theme_dir("corporate");
        std::fs::create_dir_all(&dir).expect("theme dir");
        std::fs::write(dir.join("style.css"), "body { color: red }").expect("write css");
        let theme = load(&layout, "corporate").expect("loads");
        assert_eq!(theme.name, "corporate");
        assert_eq!(theme.stylesheet, "body { color: red }");
    }

    #[test]
    fn a_missing_theme_names_the_stylesheet_path() {
        let (_dir, layout) = vault();
        let err = load(&layout, "no-such-theme").expect_err("missing");
        match err {
            RenderError::ThemeNotFound { name, path } => {
                assert_eq!(name, "no-such-theme");
                assert_eq!(path, stylesheet_path(&layout, "no-such-theme"));
                assert!(path.ends_with("themes/site/no-such-theme/style.css"));
            }
            other => panic!("expected ThemeNotFound, got {other:?}"),
        }
    }

    #[test]
    fn a_directory_without_a_stylesheet_is_a_missing_theme() {
        let (_dir, layout) = vault();
        std::fs::create_dir_all(layout.site_theme_dir("bare")).expect("theme dir");
        assert!(matches!(
            load(&layout, "bare"),
            Err(RenderError::ThemeNotFound { .. })
        ));
    }

    #[test]
    fn a_traversing_name_is_rejected_before_the_filesystem() {
        let (_dir, layout) = vault();
        assert!(matches!(
            load(&layout, "../outside"),
            Err(RenderError::InvalidThemeName { .. })
        ));
    }

    #[test]
    fn stylesheet_of_falls_back_to_the_builtin() {
        assert_eq!(
            SiteTheme::stylesheet_of(None),
            SiteTheme::builtin().stylesheet
        );
        let custom = SiteTheme {
            name: "x".to_string(),
            stylesheet: "x".to_string(),
        };
        assert_eq!(SiteTheme::stylesheet_of(Some(&custom)), "x");
    }

    #[test]
    fn write_builtin_creates_the_theme_directory_with_every_file() {
        let (_dir, layout) = vault();
        let target = layout.site_theme_dir("mine");
        write_builtin(&target).expect("writes");
        let written = std::fs::read(target.join(STYLESHEET)).expect("stylesheet written");
        assert_eq!(written, BUILTIN_FILES[0].1);
        // The copy is loadable as a vault theme without any edit.
        let theme = load(&layout, "mine").expect("loads");
        assert_eq!(theme.stylesheet, SiteTheme::builtin().stylesheet);
    }

    #[test]
    fn write_builtin_refuses_an_existing_directory() {
        let (_dir, layout) = vault();
        let target = layout.site_theme_dir("mine");
        std::fs::create_dir_all(&target).expect("pre-existing dir");
        let err = write_builtin(&target).expect_err("refused");
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    }
}
