// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Note templates with placeholder substitution (ADR 0017).
//!
//! A template is Markdown-with-frontmatter holding a fixed set of `{{…}}`
//! placeholders. Substitution is hand-rolled (no template-engine dependency):
//! a single pass replaces the four recognized placeholders and leaves anything
//! else, including unknown placeholders, verbatim. v1 ships one default
//! template, embedded here and written to disk by `init`.

use std::path::{Path, PathBuf};

/// The built-in default template.
///
/// Frontmatter carries the required `title` and an empty `tags` list; the body
/// is a single heading echoing the title.
pub const DEFAULT_TEMPLATE: &str = "\
---
title: {{title}}
tags: []
---
# {{title}}
";

/// The built-in `today` template, seeded by `init`.
///
/// The daily note is titled by its date and carries a `daily` tag; the `today`
/// command finds an existing note with today's date as title before creating one.
pub const TODAY_TEMPLATE: &str = "\
---
title: {{date}}
tags: [daily]
---
# {{date}}
";

/// The values substituted into a template's placeholders.
pub struct TemplateVars {
    /// `{{title}}` — the canonical title as typed.
    pub title: String,
    /// `{{id}}` — the note's ULID.
    pub id: String,
    /// `{{date}}` — the locally rendered creation date.
    pub date: String,
    /// `{{slug}}` — the title's slug.
    pub slug: String,
}

/// A failure resolving or reading a template.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    /// A template file could not be read (a non-`NotFound` I/O error).
    #[error("while reading template `{}`", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A named template was requested but no such file exists.
    #[error("no template `{name}` in {}", dir.display())]
    NotFound { name: String, dir: PathBuf },
    /// A template name was empty or contained a path separator.
    #[error("invalid template name `{name}`: must not be empty or contain path separators")]
    InvalidName { name: String },
}

/// Read the template at `path`, falling back to [`DEFAULT_TEMPLATE`] when the
/// file does not exist.
///
/// A missing file is normal (the vault may predate the template, or a user may
/// have removed it), so it yields the embedded default. Any other read failure
/// (e.g. a permission error) is surfaced rather than masked.
pub fn load_or_default(path: &Path) -> Result<String, TemplateError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DEFAULT_TEMPLATE.to_string()),
        Err(source) => Err(TemplateError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Load the named template `<name>.md` from `templates_dir`.
///
/// Unlike [`load_or_default`], an explicitly named template that is missing is
/// an error rather than a fallback: a user who asks for `meeting` and silently
/// gets the default would not notice a typo. The name must be a bare file stem;
/// an empty name or one containing a path separator is rejected so it cannot
/// escape the templates directory.
pub fn load_named(templates_dir: &Path, name: &str) -> Result<String, TemplateError> {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(TemplateError::InvalidName {
            name: name.to_string(),
        });
    }
    let path = templates_dir.join(format!("{name}.md"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(TemplateError::NotFound {
            name: name.to_string(),
            dir: templates_dir.to_path_buf(),
        }),
        Err(source) => Err(TemplateError::Io { path, source }),
    }
}

/// Substitute the recognized placeholders in `template`.
///
/// Scans once for `{{…}}` spans. A recognized key is replaced; an unknown key
/// is emitted verbatim (braces included) so a stray `{{foo}}` is preserved
/// rather than silently deleted. An unterminated `{{` is likewise left as-is.
pub fn render(template: &str, vars: &TemplateVars) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];

        let Some(end) = after.find("}}") else {
            // No closing braces: emit the opener literally and stop scanning.
            out.push_str("{{");
            rest = after;
            continue;
        };

        let key = after[..end].trim();
        match substitute(key, vars) {
            Some(value) => out.push_str(value),
            None => {
                out.push('{');
                out.push('{');
                out.push_str(&after[..end]);
                out.push('}');
                out.push('}');
            }
        }
        rest = &after[end + 2..];
    }

    out.push_str(rest);
    out
}

/// Map a placeholder key to its value, or `None` if it is not recognized.
fn substitute<'a>(key: &str, vars: &'a TemplateVars) -> Option<&'a str> {
    match key {
        "title" => Some(&vars.title),
        "id" => Some(&vars.id),
        "date" => Some(&vars.date),
        "slug" => Some(&vars.slug),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> TemplateVars {
        TemplateVars {
            title: "My Note".into(),
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            date: "2026-06-25".into(),
            slug: "my-note".into(),
        }
    }

    #[test]
    fn substitutes_all_placeholders() {
        let template = "{{id}} {{date}} {{slug}} {{title}}";
        assert_eq!(
            render(template, &vars()),
            "01ARZ3NDEKTSV4RRFFQ69G5FAV 2026-06-25 my-note My Note"
        );
    }

    #[test]
    fn repeated_placeholder_is_replaced_each_time() {
        assert_eq!(
            render("{{title}} / {{title}}", &vars()),
            "My Note / My Note"
        );
    }

    #[test]
    fn unknown_placeholder_is_preserved() {
        assert_eq!(
            render("{{title}} {{unknown}}", &vars()),
            "My Note {{unknown}}"
        );
    }

    #[test]
    fn unterminated_braces_preserved() {
        assert_eq!(render("a {{ b", &vars()), "a {{ b");
    }

    #[test]
    fn whitespace_inside_braces_is_tolerated() {
        assert_eq!(render("{{ title }}", &vars()), "My Note");
    }

    #[test]
    fn default_template_renders_to_valid_note() {
        let rendered = render(DEFAULT_TEMPLATE, &vars());
        insta::assert_snapshot!(rendered, @r"
        ---
        title: My Note
        tags: []
        ---
        # My Note
        ");
    }

    #[test]
    fn load_missing_falls_back_to_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        let text = load_or_default(&dir.path().join("default.md")).expect("load");
        assert_eq!(text, DEFAULT_TEMPLATE);
    }

    #[test]
    fn load_reads_existing_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("default.md");
        std::fs::write(&path, "custom {{title}}").expect("write");
        assert_eq!(load_or_default(&path).expect("load"), "custom {{title}}");
    }

    #[test]
    fn load_named_reads_the_matching_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("meeting.md"), "meeting {{title}}").expect("write");
        assert_eq!(
            load_named(dir.path(), "meeting").expect("load"),
            "meeting {{title}}"
        );
    }

    #[test]
    fn load_named_missing_is_not_found() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = load_named(dir.path(), "absent").expect_err("missing");
        assert!(matches!(err, TemplateError::NotFound { .. }));
    }

    #[test]
    fn load_named_rejects_empty_and_path_separators() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(matches!(
            load_named(dir.path(), "").expect_err("empty"),
            TemplateError::InvalidName { .. }
        ));
        assert!(matches!(
            load_named(dir.path(), "../escape").expect_err("traversal"),
            TemplateError::InvalidName { .. }
        ));
    }
}
