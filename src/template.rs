// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Note templates with placeholder substitution (ADR 0017, ADR 0034).
//!
//! A template is Markdown-with-frontmatter holding a fixed set of `{{…}}`
//! placeholders. Substitution is hand-rolled (no template-engine dependency):
//! a single pass replaces the four recognized placeholders and leaves anything
//! else, including unknown placeholders, verbatim. The templates a vault starts
//! out with live in [`vault::seed`](crate::vault::seed); this module only
//! resolves, reads, and renders them.
//!
//! The one exception to plain verbatim substitution is the frontmatter block:
//! a value substituted there sits inside YAML, which gives some placeholder
//! occurrences a syntactic context worth respecting rather than overwriting.
//! `render` splits the template into that block and the body (ADR 0034) and
//! renders each with a different strategy.

use std::path::{Path, PathBuf};

use crate::note::frontmatter;
use crate::vault::seed;

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

/// Read the template at `path`, falling back to [`seed::DEFAULT_TEMPLATE`]
/// when the file does not exist.
///
/// A missing file is normal (the vault may predate the template, or a user may
/// have removed it), so it yields the embedded default. Any other read failure
/// (e.g. a permission error) is surfaced rather than masked.
pub fn load_or_default(path: &Path) -> Result<String, TemplateError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(seed::DEFAULT_TEMPLATE.to_string())
        }
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
/// A template's leading `---`-delimited block is YAML (or is meant to become
/// YAML once the placeholders are filled in), so a value dropped there
/// verbatim can change the block's structure instead of just its content: a
/// title containing `: ` turns `title: {{title}}` into two mapping values,
/// and a title starting with `[` or `#` changes the value's parsed type or
/// deletes it as a comment. The body has no such structure to protect, so it
/// keeps the plain verbatim substitution. Splitting the template the same way
/// [`frontmatter::split`] splits a written note (same fence rules, so a
/// template's block is recognized exactly where a note's would be) lets each
/// region use the strategy its content actually needs.
pub fn render(template: &str, vars: &TemplateVars) -> String {
    let split = frontmatter::split(template);
    let Some(frontmatter_block) = split.frontmatter else {
        // No recognizable frontmatter fence: there is no YAML to protect, so
        // the whole template is body.
        return render_verbatim(template, vars);
    };

    // `frontmatter_block` and `split.body` are subslices `frontmatter::split`
    // borrowed directly from `template`, so their byte offset within it can be
    // recovered from the pointer difference. That offset is what lets the
    // fence lines themselves (and any trailing whitespace or CRLF a custom
    // template's author wrote into them) pass through untouched: only the
    // interior of the block is re-rendered.
    let frontmatter_start = offset_within(template, frontmatter_block);
    let frontmatter_end = frontmatter_start + frontmatter_block.len();
    let body_start = offset_within(template, split.body);

    let mut out = String::with_capacity(template.len());
    out.push_str(&template[..frontmatter_start]);
    out.push_str(&render_frontmatter(frontmatter_block, vars));
    out.push_str(&template[frontmatter_end..body_start]);
    out.push_str(&render_verbatim(split.body, vars));
    out
}

/// The byte offset of `sub` within `base`.
///
/// Sound only because every call site passes a `sub` that is actually a
/// subslice of `base` (both borrowed from the same template string by
/// [`frontmatter::split`]); the arithmetic never turns the result back into a
/// pointer, so it stays within what safe Rust guarantees.
fn offset_within(base: &str, sub: &str) -> usize {
    sub.as_ptr() as usize - base.as_ptr() as usize
}

/// Substitute placeholders verbatim: the body strategy, and the whole
/// template's strategy when it has no frontmatter block.
///
/// Scans once for `{{…}}` spans. A recognized key is replaced; an unknown key
/// is emitted verbatim (braces included) so a stray `{{foo}}` is preserved
/// rather than silently deleted. An unterminated `{{` is likewise left as-is.
fn render_verbatim(template: &str, vars: &TemplateVars) -> String {
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

/// Substitute placeholders in a frontmatter block interior, choosing an
/// escaping strategy per occurrence from the YAML syntax already surrounding
/// it on its line (ADR 0034).
///
/// An unrecognized key is still emitted verbatim regardless of context, same
/// as [`render_verbatim`]: a stray `{{foo}}` is not YAML the block asked for,
/// so there is nothing to escape it for.
fn render_frontmatter(block: &str, vars: &TemplateVars) -> String {
    let mut out = String::with_capacity(block.len());
    let mut rest = block;
    // The byte offset of `rest`'s start within `block`, needed because a
    // placeholder's escaping strategy depends on where it sits on its own
    // line, not on its position within `rest`.
    let mut consumed = 0usize;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];

        let Some(end) = after.find("}}") else {
            out.push_str("{{");
            consumed += start + 2;
            rest = after;
            continue;
        };

        let key = after[..end].trim();
        let span_start = consumed + start;
        let span_end = span_start + 2 + end + 2;

        match substitute(key, vars) {
            None => {
                out.push('{');
                out.push('{');
                out.push_str(&after[..end]);
                out.push('}');
                out.push('}');
            }
            Some(value) => {
                out.push_str(&render_in_frontmatter_context(
                    block, span_start, span_end, value,
                ));
            }
        }

        consumed = span_end;
        rest = &after[end + 2..];
    }

    out.push_str(rest);
    out
}

/// Format `value` for one placeholder occurrence, given the byte span
/// `[span_start, span_end)` of its `{{…}}` text within `block`.
///
/// The occurrence's line and its position on that line decide the row of the
/// context matrix (ADR 0034): inside an already-open quote, the value is
/// escaped for that quote style; otherwise, when the placeholder is the
/// line's entire mapping value, it becomes a YAML-safe scalar; anything else
/// (embedded in a bare plain value, inside a flow collection, …) has no
/// lossless escape available and passes through verbatim, same as the body.
fn render_in_frontmatter_context(
    block: &str,
    span_start: usize,
    span_end: usize,
    value: &str,
) -> String {
    let line_start = block[..span_start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = block[span_end..]
        .find('\n')
        .map_or(block.len(), |i| span_end + i);
    let prefix = &block[line_start..span_start];
    let suffix = &block[span_end..line_end];

    match quote_state_at(prefix) {
        QuoteState::Double => escape_double_quoted(value),
        QuoteState::Single => escape_single_quoted(value),
        QuoteState::None if is_bare_whole_value(prefix, suffix) => yaml_scalar(value),
        QuoteState::None => value.to_string(),
    }
}

/// Whether a YAML quote is open at the end of `line_prefix`, and which kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    None,
    Single,
    Double,
}

/// Scan `line_prefix` (a line's text up to but not including a placeholder)
/// left to right, tracking YAML quote state exactly as the YAML spec defines
/// it for the two quote styles: a single quote toggles the state, except that
/// `''` inside a single-quoted scalar is the escape for a literal `'` rather
/// than the closing quote; a double quote toggles the state, except that
/// `\"` is the escape for a literal `"` (in fact any `\x` is an escape pair,
/// since that is enough to keep `\"` from being misread as a close without
/// needing to know every double-quote escape YAML defines).
fn quote_state_at(line_prefix: &str) -> QuoteState {
    let mut state = QuoteState::None;
    let mut chars = line_prefix.chars().peekable();
    while let Some(c) = chars.next() {
        match state {
            QuoteState::None => {
                if c == '\'' {
                    state = QuoteState::Single;
                } else if c == '"' {
                    state = QuoteState::Double;
                }
            }
            QuoteState::Single => {
                if c == '\'' {
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                    } else {
                        state = QuoteState::None;
                    }
                }
            }
            QuoteState::Double => {
                if c == '\\' {
                    chars.next();
                } else if c == '"' {
                    state = QuoteState::None;
                }
            }
        }
    }
    state
}

/// Whether a placeholder, isolated into `prefix` and `suffix` (a line's text
/// before and after it), is the entire scalar value of a `key:` mapping line:
/// `prefix` ends with a colon-separated key and only whitespace after it, and
/// `suffix` is whitespace only.
fn is_bare_whole_value(prefix: &str, suffix: &str) -> bool {
    let Some(colon) = prefix.rfind(':') else {
        return false;
    };
    let key = &prefix[..colon];
    !key.trim().is_empty()
        && !key.contains(':')
        && prefix[colon + 1..].chars().all(char::is_whitespace)
        && suffix.chars().all(char::is_whitespace)
}

/// Escape `value` for embedding inside a YAML double-quoted scalar that the
/// template already opened: `\` and `"` are escaped so they cannot be misread
/// as the string's own syntax, and any control character (a raw newline
/// included) is escaped since a double-quoted scalar cannot contain one
/// unescaped.
fn escape_double_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Escape `value` for embedding inside a YAML single-quoted scalar that the
/// template already opened: the only escape a single-quoted scalar has is
/// doubling a literal `'`, so that is the only substitution needed.
fn escape_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

/// Format `value` as a standalone YAML scalar, for a placeholder that is the
/// entire value of a mapping line.
///
/// Plain-vs-quoted is not reimplemented here: `serde_yaml_ng::to_string`
/// already embodies that judgment (and is the same crate
/// `frontmatter::parse_block` parses with), so asking it for `value`'s
/// serialization and trimming its trailing document newline reuses that
/// judgment instead of duplicating YAML's plain-scalar grammar by hand. A
/// value containing a newline or other control character is handled before
/// that call: `serde_yaml_ng` represents those with a multi-line block
/// scalar, which would splice extra lines into a frontmatter block meant to
/// hold one line per field, so those go straight to the hand-rolled
/// double-quoted escape instead. The same escape is the fallback if
/// `serde_yaml_ng` ever answers with more than one line for a control-free
/// value; that check is defensive rather than a path any input in the test
/// suite has been observed to hit.
fn yaml_scalar(value: &str) -> String {
    if value.chars().any(char::is_control) {
        return format!("\"{}\"", escape_double_quoted(value));
    }

    let mut serialized = serde_yaml_ng::to_string(&value).expect("a string always serializes");
    if serialized.ends_with('\n') {
        serialized.pop();
    }
    if serialized.contains('\n') {
        return format!("\"{}\"", escape_double_quoted(value));
    }
    serialized
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
        let rendered = render(seed::DEFAULT_TEMPLATE, &vars());
        insta::assert_snapshot!(rendered, @r"
        ---
        title: My Note
        tags: []
        ---
        # My Note
        ");
    }

    /// `vars()` with a caller-chosen title, everything else held fixed.
    fn vars_with_title(title: &str) -> TemplateVars {
        TemplateVars {
            title: title.to_string(),
            ..vars()
        }
    }

    /// Render `title: {{title}}` with `title` and assert the frontmatter value
    /// `serde_yaml_ng` parses back out equals `title` exactly: the round-trip
    /// contract [`yaml_scalar`] must uphold for every value, not just the ones
    /// that stay plain.
    fn assert_title_round_trips(title: &str) {
        let template = "---\ntitle: {{title}}\n---\n";
        let rendered = render(template, &vars_with_title(title));

        let split = frontmatter::split(&rendered);
        let block = split.frontmatter.unwrap_or_else(|| {
            panic!("rendered template lost its frontmatter fence: {rendered:?}")
        });
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(block).unwrap_or_else(|e| {
            panic!("rendered frontmatter is not valid YAML: {rendered:?}: {e}")
        });
        let parsed_title = value.get("title").and_then(serde_yaml_ng::Value::as_str);
        assert_eq!(
            parsed_title,
            Some(title),
            "title did not round-trip through rendered frontmatter {rendered:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Context matrix (ADR 0034): bare whole value.
    // ---------------------------------------------------------------------

    #[test]
    fn bare_whole_value_plain_safe_title_stays_unquoted() {
        let rendered = render("---\ntitle: {{title}}\n---\n", &vars_with_title("My Note"));
        assert_eq!(rendered, "---\ntitle: My Note\n---\n");
    }

    #[test]
    fn bare_whole_value_tolerates_indentation_and_extra_spacing() {
        let rendered = render(
            "---\n  title:   {{title}}   \n---\n",
            &vars_with_title("My Note"),
        );
        // The value is still recognized as the whole scalar despite the
        // padding, so it is formatted (here, left plain) rather than left as
        // raw placeholder text.
        assert_eq!(rendered, "---\n  title:   My Note   \n---\n");
    }

    #[test]
    fn bare_whole_value_nasty_titles_round_trip() {
        for title in [
            "Q3: Planning kickoff",
            "[draft] roadmap",
            "#hashtag first",
            "He said \"go\"",
            "it's",
            "back\\slash",
            "line1\nline2",
            "  leading and trailing  ",
            "",
            "true",
            "2026-07-06",
            "~",
            "*anchor",
            "&ref",
            "{flow}",
            "- dash first",
            "Überblick: Sömmer",
        ] {
            assert_title_round_trips(title);
        }
    }

    // ---------------------------------------------------------------------
    // Context matrix: inside an already-open double-quoted string.
    // ---------------------------------------------------------------------

    #[test]
    fn double_quoted_whole_value_is_escaped_for_that_quote() {
        let rendered = render(
            "---\ntitle: \"{{title}}\"\n---\n",
            &vars_with_title("He said \"go\""),
        );
        assert_eq!(rendered, "---\ntitle: \"He said \\\"go\\\"\"\n---\n");
        assert_title_round_trips_within(&rendered, "He said \"go\"");
    }

    #[test]
    fn double_quoted_embedded_value_is_escaped_in_place() {
        let rendered = render(
            "---\ntitle: \"Meeting {{title}}\"\n---\n",
            &vars_with_title("back\\slash"),
        );
        assert_eq!(rendered, "---\ntitle: \"Meeting back\\\\slash\"\n---\n");
        assert_title_round_trips_within(&rendered, "Meeting back\\slash");
    }

    // ---------------------------------------------------------------------
    // Context matrix: inside an already-open single-quoted string.
    // ---------------------------------------------------------------------

    #[test]
    fn single_quoted_whole_value_doubles_interior_quotes() {
        let rendered = render("---\ntitle: '{{title}}'\n---\n", &vars_with_title("it's"));
        assert_eq!(rendered, "---\ntitle: 'it''s'\n---\n");
        assert_title_round_trips_within(&rendered, "it's");
    }

    #[test]
    fn single_quoted_embedded_value_doubles_interior_quotes() {
        let rendered = render(
            "---\ntitle: 'Meeting {{title}}'\n---\n",
            &vars_with_title("it's"),
        );
        assert_eq!(rendered, "---\ntitle: 'Meeting it''s'\n---\n");
        assert_title_round_trips_within(&rendered, "Meeting it's");
    }

    /// Like [`assert_title_round_trips`], but for a pre-rendered frontmatter
    /// where the expected value is the composed line's full text rather than
    /// the bare placeholder (the template embedded the placeholder inside
    /// surrounding text or an existing quote, so what round-trips is the
    /// composed string, not the substituted value alone).
    fn assert_title_round_trips_within(rendered: &str, expected_title: &str) {
        let split = frontmatter::split(rendered);
        let block = split.frontmatter.unwrap_or_else(|| {
            panic!("rendered template lost its frontmatter fence: {rendered:?}")
        });
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(block).unwrap_or_else(|e| {
            panic!("rendered frontmatter is not valid YAML: {rendered:?}: {e}")
        });
        assert_eq!(
            value.get("title").and_then(serde_yaml_ng::Value::as_str),
            Some(expected_title)
        );
    }

    // ---------------------------------------------------------------------
    // Context matrix: anything else (no lossless escape available).
    // ---------------------------------------------------------------------

    #[test]
    fn embedded_in_plain_value_is_verbatim() {
        let rendered = render(
            "---\ntitle: Meeting {{title}} notes\n---\n",
            &vars_with_title("Q3: Plan"),
        );
        // This line is not valid YAML once substituted (a bare `: ` inside a
        // plain scalar), same as before ADR 0034: this context has no
        // lossless escape, so it is left for a later validate-before-write
        // pass to catch rather than silently altered.
        assert_eq!(rendered, "---\ntitle: Meeting Q3: Plan notes\n---\n");
    }

    #[test]
    fn flow_collection_element_is_verbatim() {
        let rendered = render(
            "---\ntags: [x, {{slug}}]\n---\n",
            &TemplateVars {
                slug: "a,b".into(),
                ..vars()
            },
        );
        assert_eq!(rendered, "---\ntags: [x, a,b]\n---\n");
    }

    // ---------------------------------------------------------------------
    // Cross-cutting behavior the context split must preserve.
    // ---------------------------------------------------------------------

    #[test]
    fn body_placeholder_is_never_escaped() {
        let rendered = render(
            "---\ntitle: {{title}}\n---\n# {{title}}\n",
            &vars_with_title("Q3: Plan"),
        );
        assert!(
            rendered.ends_with("# Q3: Plan\n"),
            "body was altered: {rendered:?}"
        );
    }

    #[test]
    fn today_template_date_placeholder_stays_plain() {
        // `vars().date` ("2026-06-25") is a bare whole value like `title` in
        // the default template; asserting the exact rendering (rather than
        // just that it parses) is what "no snapshot churn" means here — the
        // ADR 0034 split must not change this template's known-good output.
        assert_eq!(
            render(seed::TODAY_TEMPLATE, &vars()),
            "---\ntitle: 2026-06-25\ntags: [daily]\n---\n# 2026-06-25\n"
        );
    }

    #[test]
    fn no_frontmatter_template_is_entirely_verbatim() {
        // No opening `---` fence at all: the whole template is body, so a
        // title that would need escaping inside a frontmatter block is
        // substituted untouched, exactly as before ADR 0034.
        assert_eq!(
            render("# {{title}}\n", &vars_with_title("Q3: Plan")),
            "# Q3: Plan\n"
        );
    }

    #[test]
    fn unknown_placeholder_in_frontmatter_is_preserved() {
        let rendered = render(
            "---\ntitle: {{title}}\nother: {{unknown}}\n---\n",
            &vars_with_title("My Note"),
        );
        assert_eq!(rendered, "---\ntitle: My Note\nother: {{unknown}}\n---\n");
    }

    #[test]
    fn default_template_with_nasty_title_parses_as_a_real_note() {
        let rendered = render(
            seed::DEFAULT_TEMPLATE,
            &vars_with_title("Q3: Planning kickoff"),
        );
        let note = crate::note::Note::parse(
            std::path::PathBuf::from(
                "/vault/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-q3-planning-kickoff.md",
            ),
            &rendered,
            None,
        )
        .expect("the default template with a YAML-special title still parses as a note");
        assert_eq!(note.title, "Q3: Planning kickoff");
    }

    #[test]
    fn load_missing_falls_back_to_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        let text = load_or_default(&dir.path().join("default.md")).expect("load");
        assert_eq!(text, seed::DEFAULT_TEMPLATE);
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
