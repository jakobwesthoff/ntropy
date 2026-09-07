// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Render themes: selecting one, and loading its source (ADR 0045).
//!
//! A theme is a Typst source file at `<vault>/.ntropy/themes/<name>.typ`,
//! emitted after the engine's own prelude so its definitions shadow the
//! built-in ones. Selection and loading are separated: [`select`] is a pure
//! decision over the two places a name can come from, and [`load`] is the one
//! step that touches the filesystem, so precedence is testable without a vault
//! on disk.
//!
//! Absence is not an error. A vault that configures no theme, and an
//! invocation that overrides one with [`DEFAULT_THEME`], both yield `None`,
//! which the engine renders exactly as it always has.

use std::path::{Path, PathBuf};

use super::RenderError;

/// The reserved theme name that selects the engine's built-in look.
///
/// It is how `--theme` overrides a vault-configured theme back to the default;
/// a file of this name in the themes directory is therefore never loaded.
pub const DEFAULT_THEME: &str = "default";

/// A loaded theme: its name, and the Typst source emitted after the prelude.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// The name as selected, for error messages and reports.
    pub name: String,
    /// The file's contents, spliced into the document verbatim.
    pub source: String,
}

/// Decide which theme name a render uses, or `None` for the built-in look.
///
/// `flag` is the `--theme` override and `configured` the vault's
/// `[render] theme`; the flag wins where both are present. [`DEFAULT_THEME`]
/// resolves to `None` from either source, so a vault theme can be overridden
/// back to the built-in look, and configuring `theme = "default"` means what it
/// says rather than looking for a file.
pub fn select<'a>(flag: Option<&'a str>, configured: Option<&'a str>) -> Option<&'a str> {
    flag.or(configured)
        .filter(|name| !name.is_empty() && *name != DEFAULT_THEME)
}

/// Load the theme named `name` from `themes_dir`.
///
/// The name is validated first: it names one file inside the themes directory,
/// never a path reaching out of it. A missing file is an error naming the path
/// that was looked for rather than a silent fall back to the built-in look,
/// since a document in the wrong livery is worse than one that was not produced
/// (ADR 0045).
pub fn load(themes_dir: &Path, name: &str) -> Result<Theme, RenderError> {
    validate_name(name)?;
    let path = themes_dir.join(format!("{name}.{}", crate::vault::layout::THEME_EXTENSION));
    match std::fs::read_to_string(&path) {
        Ok(source) => Ok(Theme {
            name: name.to_string(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(RenderError::ThemeNotFound {
            name: name.to_string(),
            path,
        }),
        Err(source) => Err(RenderError::ThemeRead { path, source }),
    }
}

/// Require `name` to be a single, ordinary filename component.
///
/// A theme name is joined onto the themes directory, so a name carrying a
/// separator or a `..` component would address a file outside it. Rejecting the
/// shape outright keeps the themes directory the only place a theme is read
/// from, and reports the bad name rather than a confusing missing-file path
/// built from it.
pub fn validate_name(name: &str) -> Result<(), RenderError> {
    let bad = name.is_empty()
        || name.contains(std::path::MAIN_SEPARATOR)
        || name.contains('/')
        || Path::new(name).components().count() != 1
        || name.starts_with('.');
    if bad {
        return Err(RenderError::InvalidThemeName {
            name: name.to_string(),
        });
    }
    Ok(())
}

/// The path [`load`] would read for `name`, for an error message built before
/// or without a load attempt.
pub fn path_for(themes_dir: &Path, name: &str) -> PathBuf {
    themes_dir.join(format!("{name}.{}", crate::vault::layout::THEME_EXTENSION))
}

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // Selection precedence
    // =====================================================================

    #[test]
    fn nothing_configured_and_no_flag_is_the_built_in_look() {
        assert_eq!(select(None, None), None);
    }

    #[test]
    fn the_configured_theme_is_used_when_no_flag_overrides_it() {
        // The vault-wide default: a plain `ntropy render` carries no flag, so
        // the config alone decides.
        assert_eq!(select(None, Some("corporate")), Some("corporate"));
    }

    #[test]
    fn the_flag_wins_over_the_configured_theme() {
        assert_eq!(
            select(Some("customer"), Some("corporate")),
            Some("customer")
        );
    }

    #[test]
    fn the_flag_applies_when_nothing_is_configured() {
        assert_eq!(select(Some("customer"), None), Some("customer"));
    }

    #[test]
    fn the_flag_can_override_a_configured_theme_back_to_the_built_in_look() {
        // The escape hatch: a themed vault still renders one document plain.
        assert_eq!(select(Some(DEFAULT_THEME), Some("corporate")), None);
    }

    #[test]
    fn configuring_the_reserved_name_means_the_built_in_look() {
        // `theme = "default"` reads as what it says; no file is looked for.
        assert_eq!(select(None, Some(DEFAULT_THEME)), None);
    }

    #[test]
    fn an_empty_name_from_either_source_is_the_built_in_look() {
        // An empty TOML value is the config equivalent of saying nothing.
        assert_eq!(select(None, Some("")), None);
        assert_eq!(select(Some(""), Some("corporate")), None);
    }

    // =====================================================================
    // Name validation
    // =====================================================================

    #[test]
    fn an_ordinary_name_validates() {
        assert!(validate_name("corporate").is_ok());
        assert!(validate_name("customer-facing").is_ok());
        assert!(validate_name("theme_2").is_ok());
    }

    #[test]
    fn a_name_with_a_separator_is_rejected() {
        // Would address a file outside the themes directory.
        let err = validate_name("sub/corporate").expect_err("a separator is rejected");
        assert!(
            err.to_string().contains("sub/corporate"),
            "the error names the bad value: {err}"
        );
    }

    #[test]
    fn a_traversing_name_is_rejected() {
        assert!(validate_name("..").is_err());
        assert!(validate_name("../../etc/passwd").is_err());
    }

    #[test]
    fn an_empty_or_hidden_name_is_rejected() {
        assert!(validate_name("").is_err());
        assert!(validate_name(".hidden").is_err());
    }

    // =====================================================================
    // Loading
    // =====================================================================

    /// A themes directory holding one theme file.
    fn themes_dir_with(name: &str, source: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(format!("{name}.typ")), source).expect("write theme");
        dir
    }

    #[test]
    fn a_present_theme_loads_its_source_verbatim() {
        let source = "#let note(title: none, frontmatter: (:), paper: \"a4\", body) = body\n";
        let dir = themes_dir_with("corporate", source);
        let theme = load(dir.path(), "corporate").expect("the theme loads");
        assert_eq!(theme.name, "corporate");
        assert_eq!(theme.source, source);
    }

    #[test]
    fn a_missing_theme_errors_naming_the_path_it_looked_for() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = load(dir.path(), "no-such-theme").expect_err("a missing theme errors");
        let message = err.to_string();
        assert!(
            message.contains("no-such-theme"),
            "the error names the theme: {message}"
        );
        match err {
            RenderError::ThemeNotFound { path, .. } => {
                assert_eq!(path, dir.path().join("no-such-theme.typ"));
            }
            other => panic!("expected ThemeNotFound, got {other:?}"),
        }
    }

    #[test]
    fn an_unreadable_theme_is_a_read_error_not_a_missing_one() {
        // A directory where the file should be: it exists, so this is not the
        // "you have not created it" case, and the distinct variant keeps the
        // two apart in the message the user sees.
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("corporate.typ")).expect("create the blocking dir");
        let err = load(dir.path(), "corporate").expect_err("a directory does not read");
        assert!(
            matches!(err, RenderError::ThemeRead { .. }),
            "expected ThemeRead, got {err:?}"
        );
    }

    #[test]
    fn loading_rejects_a_traversing_name_before_touching_the_filesystem() {
        // The parent of the themes directory holds a readable file; a
        // traversing name must not reach it.
        let parent = tempfile::tempdir().expect("temp dir");
        std::fs::write(parent.path().join("outside.typ"), "#let note(body) = body")
            .expect("write the outside file");
        let themes = parent.path().join("themes");
        std::fs::create_dir(&themes).expect("themes dir");

        let err = load(&themes, "../outside").expect_err("traversal is rejected");
        assert!(
            matches!(err, RenderError::InvalidThemeName { .. }),
            "expected InvalidThemeName, got {err:?}"
        );
    }

    #[test]
    fn path_for_matches_what_load_reads() {
        let dir = themes_dir_with("corporate", "#let note(body) = body");
        assert_eq!(
            path_for(dir.path(), "corporate"),
            dir.path().join("corporate.typ")
        );
        assert!(path_for(dir.path(), "corporate").is_file());
    }
}
