// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The icon sprite a theme's `icons/` directory becomes (ADR 0055). Every
//! `<name>.svg` in it is one `<symbol id="icon-<name>">` of a hidden inline
//! `<svg>` that every page carries, and the markup shows an icon with
//! `<svg class="icon"><use href="#icon-<name>"/></svg>`. Inlining is what
//! makes the icons work over `file://`, where a `<use>` of another file is
//! refused as cross-origin.
//!
//! A symbol keeps the file's root attributes (`viewBox`, `fill`, `stroke`,
//! and the rest) so an icon renders as it was drawn, minus what only makes
//! sense on a standalone file: the namespace, `width` and `height`, `class`
//! and `id`. The file's content between the root tags is kept as written,
//! comments outside the root dropped.

use std::collections::BTreeMap;

/// The theme directory holding the icons.
pub const ICONS_DIR: &str = "icons";

/// The prefix of a symbol id: `icons/tag.svg` is `#icon-tag`.
pub const ID_PREFIX: &str = "icon-";

/// Why a file is no icon.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IconError {
    #[error("no <svg> root element")]
    NoRoot,
    #[error("the <svg> root element is not closed")]
    Unclosed,
}

/// The icon name of a file in the icons directory: the stem of a `.svg`
/// file, or `None` for any other file (a license, say).
pub fn name_of(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(".svg")?;
    (!stem.is_empty()).then_some(stem)
}

/// The `<symbol>` for the icon `name` drawn by the SVG file `svg`.
pub fn symbol(name: &str, svg: &str) -> Result<String, IconError> {
    let root_start = svg.find("<svg").ok_or(IconError::NoRoot)?;
    let after_start = &svg[root_start + "<svg".len()..];
    let attributes_end = after_start.find('>').ok_or(IconError::Unclosed)?;
    let attributes = &after_start[..attributes_end];
    let body = &after_start[attributes_end + 1..];
    let body_end = body.rfind("</svg>").ok_or(IconError::Unclosed)?;
    let inner = collapse(&body[..body_end]);

    let mut out = format!("<symbol id=\"{ID_PREFIX}{name}\"");
    for (key, value) in attribute_pairs(attributes) {
        if matches!(key, "width" | "height" | "class" | "id") || key.starts_with("xmlns") {
            continue;
        }
        out.push_str(&format!(" {key}=\"{value}\""));
    }
    out.push('>');
    out.push_str(&inner);
    out.push_str("</symbol>");
    Ok(out)
}

/// The hidden inline sprite holding `symbols`, in name order.
pub fn sprite(symbols: &BTreeMap<String, String>) -> String {
    let mut out = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" style=\"display:none\" aria-hidden=\"true\">",
    );
    for symbol in symbols.values() {
        out.push_str(symbol);
    }
    out.push_str("</svg>");
    out
}

/// The `key="value"` pairs of an attribute string, in order. Values may be
/// single- or double-quoted; anything else is skipped.
fn attribute_pairs(attributes: &str) -> Vec<(&str, &str)> {
    let mut pairs = Vec::new();
    let mut rest = attributes;
    loop {
        rest = rest.trim_start();
        let Some(eq) = rest.find('=') else { break };
        let key = rest[..eq].trim();
        let value_start = rest[eq + 1..].trim_start();
        let Some(quote) = value_start.chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            break;
        }
        let value_body = &value_start[1..];
        let Some(end) = value_body.find(quote) else {
            break;
        };
        pairs.push((key, &value_body[..end]));
        rest = &value_body[end + 1..];
    }
    pairs
}

/// The inner markup with comments removed and runs of whitespace between
/// tags collapsed, so the sprite carries no indentation.
fn collapse(inner: &str) -> String {
    let mut without_comments = String::with_capacity(inner.len());
    let mut rest = inner;
    while let Some(start) = rest.find("<!--") {
        without_comments.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + "-->".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    without_comments.push_str(rest);
    without_comments
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("> <", "><")
        .replace(" />", "/>")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUCIDE_TAG: &str = "<!-- @license lucide-static v1.44.0 - ISC -->\n<svg\n  class=\"lucide lucide-tag\"\n  xmlns=\"http://www.w3.org/2000/svg\"\n  width=\"24\"\n  height=\"24\"\n  viewBox=\"0 0 24 24\"\n  fill=\"none\"\n  stroke=\"currentColor\"\n  stroke-width=\"2\"\n  stroke-linecap=\"round\"\n  stroke-linejoin=\"round\"\n>\n  <path d=\"M12 2h4\" />\n  <circle cx=\"7.5\" cy=\"7.5\" r=\".5\" fill=\"currentColor\" />\n</svg>\n";

    #[test]
    fn a_lucide_file_becomes_a_symbol_keeping_its_presentation() {
        assert_eq!(
            symbol("tag", LUCIDE_TAG).expect("parses"),
            "<symbol id=\"icon-tag\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 2h4\"/><circle cx=\"7.5\" cy=\"7.5\" r=\".5\" fill=\"currentColor\"/></symbol>"
        );
    }

    #[test]
    fn a_minimal_file_and_single_quotes_work_too() {
        assert_eq!(
            symbol("dot", "<svg viewBox='0 0 2 2'><circle r='1'/></svg>").expect("parses"),
            "<symbol id=\"icon-dot\" viewBox=\"0 0 2 2\"><circle r='1'/></symbol>"
        );
        assert_eq!(
            symbol(
                "x",
                "<?xml version=\"1.0\"?>\n<svg id=\"a\" xmlns:xlink=\"x\"><!-- inner --><g/></svg>"
            )
            .expect("parses"),
            "<symbol id=\"icon-x\"><g/></symbol>"
        );
    }

    #[test]
    fn a_file_without_a_root_or_unclosed_is_refused() {
        assert_eq!(symbol("a", "<p>not svg</p>"), Err(IconError::NoRoot));
        assert_eq!(
            symbol("a", "<svg viewBox='0 0 1 1'"),
            Err(IconError::Unclosed)
        );
        assert_eq!(symbol("a", "<svg><g/>"), Err(IconError::Unclosed));
    }

    #[test]
    fn names_come_from_svg_stems_only() {
        assert_eq!(name_of("tag.svg"), Some("tag"));
        assert_eq!(name_of("chevron-right.svg"), Some("chevron-right"));
        assert_eq!(name_of("LICENSE"), None);
        assert_eq!(name_of(".svg"), None);
        assert_eq!(name_of("tag.SVG"), None);
    }

    #[test]
    fn the_sprite_is_hidden_and_ordered_by_name() {
        let mut symbols = BTreeMap::new();
        symbols.insert("b".to_string(), "<symbol id=\"icon-b\"/>".to_string());
        symbols.insert("a".to_string(), "<symbol id=\"icon-a\"/>".to_string());
        assert_eq!(
            sprite(&symbols),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" style=\"display:none\" aria-hidden=\"true\"><symbol id=\"icon-a\"/><symbol id=\"icon-b\"/></svg>"
        );
    }
}
