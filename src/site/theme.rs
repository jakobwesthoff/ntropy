// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Site themes: the built-in one and those a vault holds (ADR 0048).
//!
//! A site theme is a directory `<vault>/.ntropy/themes/site/<name>/` of
//! stylesheets and static assets over an HTML structure that is ntropy's own.
//! Its entry point is `style.css`; `icons/*.svg` become the icon sprite
//! every page inlines ([`super::icons`]), layered by name over the built-in
//! icons; every file in the directory is shipped beside the pages. The
//! built-in theme is the same layout, everything under `src/site/theme/`,
//! embedded in the binary by the build script; `site theme init` writes it
//! into a vault as the starting point for a custom theme.
//!
//! Selection reuses the render themes' rule: `--theme` over `[site] theme`,
//! with the reserved name `default` meaning the built-in theme from either
//! source ([`crate::render::theme::select`]).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use super::embedded::{self, Asset};
use super::icons;
use crate::render::RenderError;
use crate::render::theme::validate_name;
use crate::vault::layout::Layout;

/// The stylesheet a site theme directory must contain.
pub const STYLESHEET: &str = "style.css";

/// The files of the built-in theme: every file under `src/site/theme/`,
/// embedded by the build script so a file added there ships without being
/// listed. The stylesheet, the fonts it declares, and their license.
pub const BUILTIN_FILES: &[Asset] = include!(concat!(env!("OUT_DIR"), "/site_theme_files.rs"));

/// A site theme ready to use: its name, its stylesheet, and its icons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteTheme {
    /// The name as selected, for reports; `default` for the built-in theme.
    pub name: String,
    /// The contents of `style.css`.
    pub stylesheet: String,
    /// The icons by name, each as its `<symbol>`: the built-in set with the
    /// theme's own `icons/*.svg` layered over it by name.
    pub icons: BTreeMap<String, String>,
}

impl SiteTheme {
    /// The theme embedded in the binary.
    pub fn builtin() -> Self {
        SiteTheme {
            name: crate::render::theme::DEFAULT_THEME.to_string(),
            stylesheet: embedded::find(BUILTIN_FILES, STYLESHEET)
                .expect("the built-in theme has a stylesheet")
                .text(),
            icons: builtin_icons().clone(),
        }
    }

    /// `theme`, or the built-in theme when none is selected.
    pub fn selected(theme: Option<&SiteTheme>) -> SiteTheme {
        theme.cloned().unwrap_or_else(SiteTheme::builtin)
    }

    /// The hidden inline sprite of the theme's icons, what every page
    /// carries.
    pub fn sprite(&self) -> String {
        icons::sprite(&self.icons)
    }
}

/// The built-in icons, parsed once per process from the embedded
/// `icons/*.svg`; a file there that is no icon is a broken build.
fn builtin_icons() -> &'static BTreeMap<String, String> {
    static ICONS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    ICONS.get_or_init(|| {
        let prefix = format!("{}/", icons::ICONS_DIR);
        BUILTIN_FILES
            .iter()
            .filter_map(|asset| {
                let name = icons::name_of(asset.path.strip_prefix(&prefix)?)?;
                let symbol = icons::symbol(name, &asset.text())
                    .expect("the built-in icons are SVG files with a root element");
                Some((name.to_string(), symbol))
            })
            .collect()
    })
}

/// Load the site theme named `name` from the vault.
///
/// The name is validated as a single path component first. A theme
/// directory without a `style.css` is reported as a missing theme naming
/// that path, since the stylesheet is what makes a directory a theme. The
/// `.svg` files of an `icons/` directory beside it replace or add icons by
/// name; one that is no SVG fails the load naming the file.
pub fn load(layout: &Layout, name: &str) -> Result<SiteTheme, RenderError> {
    validate_name(name)?;
    let path = stylesheet_path(layout, name);
    let stylesheet = match std::fs::read_to_string(&path) {
        Ok(stylesheet) => stylesheet,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(RenderError::ThemeNotFound {
                name: name.to_string(),
                path,
            });
        }
        Err(source) => return Err(RenderError::ThemeRead { path, source }),
    };
    let mut icons = builtin_icons().clone();
    let icons_dir = layout.site_theme_dir(name).join(icons::ICONS_DIR);
    let entries = match std::fs::read_dir(&icons_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SiteTheme {
                name: name.to_string(),
                stylesheet,
                icons,
            });
        }
        Err(source) => {
            return Err(RenderError::ThemeRead {
                path: icons_dir,
                source,
            });
        }
    };
    for entry in entries {
        let path = entry
            .map_err(|source| RenderError::ThemeRead {
                path: icons_dir.clone(),
                source,
            })?
            .path();
        let Some(icon) = path
            .file_name()
            .and_then(|file| file.to_str())
            .and_then(icons::name_of)
        else {
            continue;
        };
        let svg = std::fs::read_to_string(&path).map_err(|source| RenderError::ThemeRead {
            path: path.clone(),
            source,
        })?;
        let symbol = icons::symbol(icon, &svg).map_err(|reason| RenderError::ThemeIcon {
            path: path.clone(),
            reason: reason.to_string(),
        })?;
        icons.insert(icon.to_string(), symbol);
    }
    Ok(SiteTheme {
        name: name.to_string(),
        stylesheet,
        icons,
    })
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
    for asset in BUILTIN_FILES {
        let path = dir.join(asset.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, asset.contents())?;
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
        assert!(embedded::find(BUILTIN_FILES, STYLESHEET).is_some());
    }

    /// The built-in icons are exactly the `.svg` files under `icons/`, and
    /// every icon the markup references is among them.
    #[test]
    fn the_builtin_icons_come_from_the_icons_directory_and_cover_the_markup() {
        let theme = SiteTheme::builtin();
        let files = BUILTIN_FILES
            .iter()
            .filter(|asset| asset.path.starts_with("icons/") && asset.path.ends_with(".svg"))
            .count();
        assert_eq!(theme.icons.len(), files);
        for name in [
            "menu",
            "x",
            "search",
            "monitor",
            "sun",
            "moon",
            "tag",
            "chevron-right",
            "chevron-left",
            "info",
            "lightbulb",
            "message-square-warning",
            "triangle-alert",
            "octagon-alert",
        ] {
            assert!(theme.icons.contains_key(name), "{name} is missing");
        }
        let sprite = theme.sprite();
        assert!(
            sprite.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" style=\"display:none\"")
        );
        assert!(
            sprite.contains("<symbol id=\"icon-search\" viewBox=\"0 0 24 24\""),
            "{sprite}"
        );
        assert!(
            BUILTIN_FILES
                .iter()
                .any(|asset| asset.path == "icons/LICENSE")
        );
    }

    #[test]
    fn a_vault_theme_loads_its_stylesheet_and_inherits_the_builtin_icons() {
        let (_dir, layout) = vault();
        let dir = layout.site_theme_dir("corporate");
        std::fs::create_dir_all(&dir).expect("theme dir");
        std::fs::write(dir.join("style.css"), "body { color: red }").expect("write css");
        let theme = load(&layout, "corporate").expect("loads");
        assert_eq!(theme.name, "corporate");
        assert_eq!(theme.stylesheet, "body { color: red }");
        assert_eq!(theme.icons, SiteTheme::builtin().icons);
    }

    #[test]
    fn a_vault_theme_layers_its_icons_over_the_builtin_ones_by_name() {
        let (_dir, layout) = vault();
        let dir = layout.site_theme_dir("corporate");
        std::fs::create_dir_all(dir.join("icons")).expect("theme dirs");
        std::fs::write(dir.join("style.css"), "body {}").expect("write css");
        std::fs::write(
            dir.join("icons/tag.svg"),
            "<svg viewBox=\"0 0 1 1\"><g/></svg>",
        )
        .expect("override");
        std::fs::write(
            dir.join("icons/brand.svg"),
            "<svg viewBox=\"0 0 2 2\"><g/></svg>",
        )
        .expect("addition");
        std::fs::write(dir.join("icons/LICENSE"), "MIT").expect("not an icon");
        let theme = load(&layout, "corporate").expect("loads");
        assert_eq!(theme.icons.len(), SiteTheme::builtin().icons.len() + 1);
        assert_eq!(
            theme.icons["tag"],
            "<symbol id=\"icon-tag\" viewBox=\"0 0 1 1\"><g/></symbol>"
        );
        assert_eq!(
            theme.icons["brand"],
            "<symbol id=\"icon-brand\" viewBox=\"0 0 2 2\"><g/></symbol>"
        );
        assert_eq!(theme.icons["search"], SiteTheme::builtin().icons["search"]);
    }

    #[test]
    fn a_theme_icon_that_is_no_svg_fails_the_load_naming_it() {
        let (_dir, layout) = vault();
        let dir = layout.site_theme_dir("corporate");
        std::fs::create_dir_all(dir.join("icons")).expect("theme dirs");
        std::fs::write(dir.join("style.css"), "body {}").expect("write css");
        std::fs::write(dir.join("icons/broken.svg"), "<p>nope</p>").expect("broken");
        match load(&layout, "corporate") {
            Err(RenderError::ThemeIcon { path, reason }) => {
                assert!(path.ends_with("icons/broken.svg"));
                assert_eq!(reason, "no <svg> root element");
            }
            other => panic!("expected ThemeIcon, got {other:?}"),
        }
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
    fn selected_falls_back_to_the_builtin() {
        assert_eq!(SiteTheme::selected(None), SiteTheme::builtin());
        let custom = SiteTheme {
            name: "x".to_string(),
            stylesheet: "x".to_string(),
            icons: BTreeMap::new(),
        };
        assert_eq!(SiteTheme::selected(Some(&custom)), custom);
    }

    #[test]
    fn write_builtin_creates_the_theme_directory_with_every_file() {
        let (_dir, layout) = vault();
        let target = layout.site_theme_dir("mine");
        write_builtin(&target).expect("writes");
        let written = std::fs::read(target.join(STYLESHEET)).expect("stylesheet written");
        assert_eq!(written, SiteTheme::builtin().stylesheet.as_bytes());
        for asset in BUILTIN_FILES {
            assert!(
                target.join(asset.path).is_file(),
                "{} is written",
                asset.path
            );
        }
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

    /// The skill's theme reference carries the built-in theme's token,
    /// icon, and class names itself, since it is installed outside the
    /// repository; every name it gives has to exist here, so the copy
    /// cannot drift silently.
    #[test]
    fn the_skill_names_only_tokens_icons_and_classes_the_theme_has() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let skill = std::fs::read_to_string(root.join("skills/ntropy/references/site-themes.md"))
            .expect("the skill's theme reference exists");
        let stylesheet = SiteTheme::builtin().stylesheet;
        let icons: Vec<String> = std::fs::read_dir(root.join("src/site/theme/icons"))
            .expect("the icons directory exists")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        // Everything that emits page markup, so a class the skill names is
        // found wherever it is written.
        let markup: String = [
            "src/site/templates/page.html",
            "src/site/templates/note.html",
            "src/site/nav.rs",
            "src/render/html/emitter.rs",
            "site/src/search/ui.tsx",
        ]
        .iter()
        .map(|path| std::fs::read_to_string(root.join(path)).expect(path))
        .collect::<Vec<_>>()
        .join("\n");

        let tokens = regex::Regex::new(r"`(--[a-z-]+)`").expect("pattern");
        for token in tokens.captures_iter(&skill).map(|c| c[1].to_string()) {
            // The Shiki token colors come from the highlighter, not the theme.
            if token.starts_with("--shiki-") {
                continue;
            }
            assert!(
                stylesheet.contains(&format!("{token}:")),
                "the skill names {token}, which style.css does not declare"
            );
        }

        let icon_section = skill
            .split("## Icons")
            .nth(1)
            .and_then(|rest| rest.split("## Markup").next())
            .expect("the icons section");
        let names = regex::Regex::new(r"`([a-z][a-z-]*)`").expect("pattern");
        for name in names.captures_iter(icon_section).map(|c| c[1].to_string()) {
            if name == "icons" {
                continue;
            }
            assert!(
                icons.contains(&format!("{name}.svg")),
                "the skill names the icon {name}, which the theme does not ship"
            );
        }

        let markup_section = skill.split("## Markup").nth(1).expect("the markup section");
        let classes = regex::Regex::new(r"`[a-z0-9]*\.([a-z][a-z0-9-]*)").expect("pattern");
        for class in classes
            .captures_iter(markup_section)
            .map(|c| c[1].to_string())
        {
            assert!(
                markup.contains(&class),
                "the skill names the class .{class}, which no page carries"
            );
        }
    }
}
