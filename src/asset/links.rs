// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Finding and rewriting the links that reach files (ADR 0028).
//!
//! What is a link is decided by the Markdown parser the renderers use, with
//! their options, so a check sees exactly the links a rendered note shows:
//! reference-style, collapsed and shortcut links, and angle-bracket targets
//! for names with spaces, as much as the inline form. Code spans, code
//! blocks and raw HTML hold no link for the parser, and none here.
//!
//! Rewriting a folder name is a different job. It follows the folder
//! wherever the body spells it, including raw `<img>` tags and reference
//! definitions nothing uses, since the ULID in the name makes a false hit
//! implausible and a link left behind would break.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Parser, Tag};

use crate::link;
use crate::render::markdown::parser_options;
use crate::vault::layout::ASSETS_DIR;

/// A link or image in a note body whose target is a file in the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalLink {
    /// The target as the parser reads it: escapes and entities resolved,
    /// any `#fragment` still attached.
    pub dest: String,
    /// Whether the link is an image, `![..](..)`.
    pub is_image: bool,
    /// The byte range of the whole link in the body.
    pub range: Range<usize>,
}

impl LocalLink {
    /// The target without its `#fragment`, the file the link names.
    pub fn path(&self) -> &str {
        self.dest
            .split_once('#')
            .map_or(&self.dest, |(path, _)| path)
    }
}

/// Every link and image of `body` that points at a file: no scheme, not a
/// fragment of the same note, not a note link.
pub fn local_links(body: &str) -> Vec<LocalLink> {
    Parser::new_ext(body, parser_options())
        .into_offset_iter()
        .filter_map(|(event, range)| {
            let (link_type, dest_url, is_image) = match event {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    ..
                }) => (link_type, dest_url, false),
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    ..
                }) => (link_type, dest_url, true),
                _ => return None,
            };
            let autolink = matches!(link_type, LinkType::Autolink | LinkType::Email);
            if autolink || !names_a_file(&dest_url) {
                return None;
            }
            Some(LocalLink {
                dest: dest_url.into_string(),
                is_image,
                range,
            })
        })
        .collect()
}

/// Whether `dest` names a file of the vault rather than a URL, a heading of
/// the same note or another note.
pub fn names_a_file(dest: &str) -> bool {
    !dest.is_empty()
        && !dest.starts_with('#')
        && !dest.starts_with("//")
        && !has_scheme(dest)
        && !link::is_note_target(dest)
}

/// Whether `dest` starts with a URI scheme, `https:` or `mailto:`.
fn has_scheme(dest: &str) -> bool {
    let Some((scheme, _)) = dest.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

/// One `_assets/<name>` in a body, outside code, in any syntax: a link, an
/// image, a reference definition, a raw HTML attribute or plain text.
///
/// This is the one matcher for what a body says about an asset folder. A link
/// the parser finds is [`local_links`], and a rewrite, the `delete` scan for
/// notes that link a folder and the checks of `reconcile` all read the same
/// occurrences from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The folder name, the text after `_assets/` up to the next `/` or the
    /// end of the reference.
    pub name: String,
    /// Where the name stands in the body.
    pub name_range: Range<usize>,
    /// The whole reference, from `_assets` to the end of the path.
    pub token: Range<usize>,
    /// Whether a `/` follows the name: a path below the folder, or the folder
    /// written with a trailing slash. A name without one is no folder path,
    /// since it may as well be a file directly in `_assets/`.
    pub slash: bool,
}

/// Every `_assets/<name>` in `body` outside code, in order. The `_assets`
/// component matches ignoring ASCII case.
pub fn references(body: &str) -> Vec<Reference> {
    let masked = link::masked_ranges(body);
    let bytes = body.as_bytes();
    let lead = ASSETS_DIR.len() + 1;
    let mut found = Vec::new();
    let mut at = 0;
    while at + lead < bytes.len() {
        let starts = bytes[at..at + ASSETS_DIR.len()].eq_ignore_ascii_case(ASSETS_DIR.as_bytes())
            && bytes[at + ASSETS_DIR.len()] == b'/'
            && (at == 0 || !is_name_byte(bytes[at - 1]))
            && !link::is_masked(&masked, at);
        if !starts {
            at += 1;
            continue;
        }
        let name_start = at + lead;
        let name_end = (name_start..bytes.len())
            .find(|&i| ends_name(bytes[i]))
            .unwrap_or(bytes.len());
        let slash = bytes.get(name_end) == Some(&b'/');
        let token_end = (name_end..bytes.len())
            .find(|&i| ends_reference(bytes[i]))
            .unwrap_or(bytes.len());
        if name_end > name_start {
            found.push(Reference {
                name: body[name_start..name_end].to_owned(),
                name_range: name_start..name_end,
                token: at..token_end,
                slash,
            });
        }
        at = token_end.max(at + 1);
    }
    found
}

/// The byte ranges of `body` the parser reads as raw HTML, an HTML block or
/// an inline tag.
pub fn html_ranges(body: &str) -> Vec<Range<usize>> {
    Parser::new_ext(body, parser_options())
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Html(_) | Event::InlineHtml(_) => Some(range),
            _ => None,
        })
        .collect()
}

/// Rewrite the folder name `from` to `to` after `_assets/` everywhere in
/// `body` outside code, returning the new body and the number of places
/// changed, or `None` when `from` does not occur.
///
/// Only a reference with a `/` after the name is rewritten, a path below the
/// folder or the folder with a trailing slash. A bare `_assets/<name>` may be a
/// file directly in `_assets/` and is left as it is.
pub fn relink_folder(body: &str, from: &str, to: &str) -> Option<(String, usize)> {
    let mut rewritten = String::new();
    let mut cursor = 0;
    let mut count = 0;
    for reference in references(body)
        .iter()
        .filter(|reference| reference.slash && reference.name == from)
    {
        rewritten.push_str(&body[cursor..reference.name_range.start]);
        rewritten.push_str(to);
        cursor = reference.name_range.end;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    rewritten.push_str(&body[cursor..]);
    Some((rewritten, count))
}

/// Whether `byte` ends a folder name in a reference: the next path segment, or
/// anything that ends a link target or an attribute value.
fn ends_name(byte: u8) -> bool {
    byte == b'/' || ends_reference(byte)
}

/// Whether `byte` ends a reference.
fn ends_reference(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b')' | b'>' | b'"' | b'\'' | b'#' | b'?' | b']')
}

/// Whether `byte` can be part of a folder or directory name as written in a
/// link, which is what keeps `my_assets/` and `<folder>-more/` from matching.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.') || !byte.is_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dests(body: &str) -> Vec<String> {
        local_links(body).into_iter().map(|l| l.dest).collect()
    }

    #[test]
    fn an_inline_link_and_an_image_are_found() {
        let links = local_links("[doc](_assets/f/a.pdf) and ![pic](_assets/f/b.png)");
        let found: Vec<(&str, bool)> = links
            .iter()
            .map(|l| (l.dest.as_str(), l.is_image))
            .collect();
        assert_eq!(
            found,
            [("_assets/f/a.pdf", false), ("_assets/f/b.png", true)]
        );
    }

    #[test]
    fn the_range_covers_the_whole_link() {
        let body = "see [doc](a.pdf) here";
        let link = &local_links(body)[0];
        assert_eq!(&body[link.range.clone()], "[doc](a.pdf)");
    }

    #[test]
    fn a_reference_style_link_is_found_with_its_definition_target() {
        let body = "![pic][r]\n\n[r]: _assets/f/b.png\n";
        assert_eq!(dests(body), ["_assets/f/b.png"]);
    }

    #[test]
    fn a_collapsed_and_a_shortcut_reference_are_found() {
        let body = "[r][] and [r]\n\n[r]: _assets/f/b.png\n";
        assert_eq!(dests(body), ["_assets/f/b.png", "_assets/f/b.png"]);
    }

    #[test]
    fn an_angle_bracket_target_with_a_space_is_found() {
        assert_eq!(
            dests("![pic](<_assets/f/my pic.png>)"),
            ["_assets/f/my pic.png"]
        );
    }

    #[test]
    fn a_link_in_a_code_span_is_not_a_link() {
        assert!(dests("`[doc](a.pdf)`").is_empty());
    }

    #[test]
    fn a_link_in_a_fenced_block_is_not_a_link() {
        assert!(dests("```\n[doc](a.pdf)\n```\n").is_empty());
    }

    #[test]
    fn a_link_in_an_indented_block_is_not_a_link() {
        assert!(dests("para\n\n    [doc](a.pdf)\n").is_empty());
    }

    #[test]
    fn a_raw_html_image_is_not_a_link() {
        assert!(dests("<img src=\"_assets/f/b.png\">").is_empty());
    }

    #[test]
    fn a_url_is_not_a_local_link() {
        assert!(dests("[a](https://example.org/x.png) [b](mailto:me@example.org)").is_empty());
    }

    #[test]
    fn an_autolink_is_not_a_local_link() {
        assert!(dests("<https://example.org/x.png>").is_empty());
    }

    #[test]
    fn a_protocol_relative_url_is_not_a_local_link() {
        assert!(dests("[a](//example.org/x.png)").is_empty());
    }

    #[test]
    fn a_fragment_of_the_same_note_is_not_a_local_link() {
        assert!(dests("[a](#heading)").is_empty());
    }

    #[test]
    fn a_note_link_is_not_a_local_link() {
        assert!(dests("[a](01ARZ3NDEKTSV4RRFFQ69G5FAV-note.md#part)").is_empty());
    }

    #[test]
    fn a_link_with_no_target_is_not_a_local_link() {
        assert!(dests("[a]()").is_empty());
    }

    #[test]
    fn the_path_drops_the_fragment() {
        let link = &local_links("[a](_assets/f/doc.pdf#page=2)")[0];
        assert_eq!(link.path(), "_assets/f/doc.pdf");
    }

    const FROM: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV-old";
    const TO: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV-new";

    fn relinked(body: &str) -> Option<(String, usize)> {
        relink_folder(body, FROM, TO)
    }

    #[test]
    fn an_inline_target_follows_the_folder() {
        assert_eq!(
            relinked(&format!("![a](_assets/{FROM}/a.png)")),
            Some((format!("![a](_assets/{TO}/a.png)"), 1))
        );
    }

    #[test]
    fn a_reference_definition_follows_the_folder() {
        assert_eq!(
            relinked(&format!("![a][r]\n\n[r]: _assets/{FROM}/a.png\n")),
            Some((format!("![a][r]\n\n[r]: _assets/{TO}/a.png\n"), 1))
        );
    }

    #[test]
    fn a_raw_html_image_follows_the_folder() {
        assert_eq!(
            relinked(&format!("<img src=\"_assets/{FROM}/a.png\">")),
            Some((format!("<img src=\"_assets/{TO}/a.png\">"), 1))
        );
    }

    #[test]
    fn every_occurrence_is_counted() {
        let body = format!("[a](_assets/{FROM}/a.png) [b](_assets/{FROM}/b.png)");
        assert_eq!(relinked(&body).map(|(_, count)| count), Some(2));
    }

    #[test]
    fn the_spelling_of_the_assets_component_is_kept() {
        assert_eq!(
            relinked(&format!("[a](_ASSETS/{FROM}/a.png)")),
            Some((format!("[a](_ASSETS/{TO}/a.png)"), 1))
        );
    }

    #[test]
    fn a_path_through_the_vault_root_follows_the_folder() {
        assert_eq!(
            relinked(&format!("[a](/all-notes/_assets/{FROM}/a.png)")),
            Some((format!("[a](/all-notes/_assets/{TO}/a.png)"), 1))
        );
    }

    #[test]
    fn a_link_to_the_folder_itself_follows_it_with_its_trailing_slash() {
        assert_eq!(
            relinked(&format!("[all](_assets/{FROM}/)")),
            Some((format!("[all](_assets/{TO}/)"), 1))
        );
    }

    #[test]
    fn a_bare_name_without_a_slash_is_left_alone() {
        assert_eq!(relinked(&format!("[all](_assets/{FROM})")), None);
    }

    #[test]
    fn a_percent_encoded_slash_is_no_path_below_the_folder() {
        assert_eq!(relinked(&format!("[x](_assets/{FROM}%2Fchart.png)")), None);
    }

    #[test]
    fn references_name_every_occurrence_outside_code_with_its_ranges() {
        let body = format!(
            "![a](_assets/{FROM}/a.png) <img src=\"_assets/other/b.png\"> `_assets/code/x` [r]: _assets/loose.txt"
        );
        let found = references(&body);
        let names: Vec<&str> = found.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, [FROM, "other", "loose.txt"]);
        assert_eq!(&body[found[0].name_range.clone()], FROM);
        assert_eq!(&body[found[1].token.clone()], "_assets/other/b.png");
        assert_eq!(
            found.iter().map(|r| r.slash).collect::<Vec<_>>(),
            [true, true, false]
        );
    }

    #[test]
    fn html_ranges_cover_blocks_and_inline_tags() {
        let body = "<img src=\"_assets/a/b.png\">\n\ntext <b>bold</b> and ![x](_assets/c/d.png)";
        let ranges = html_ranges(body);
        assert!(
            ranges
                .iter()
                .any(|r| body[r.clone()].contains("_assets/a/"))
        );
        assert!(
            !ranges
                .iter()
                .any(|r| body[r.clone()].contains("_assets/c/"))
        );
    }

    #[test]
    fn code_keeps_the_old_name() {
        let body = format!("`_assets/{FROM}/a.png`\n\n```\n_assets/{FROM}/b.png\n```\n");
        assert_eq!(relinked(&body), None);
    }

    #[test]
    fn a_longer_folder_name_is_not_a_match() {
        assert_eq!(relinked(&format!("[a](_assets/{FROM}-more/a.png)")), None);
    }

    #[test]
    fn a_look_alike_assets_directory_is_not_a_match() {
        assert_eq!(relinked(&format!("[a](my_assets/{FROM}/a.png)")), None);
    }

    #[test]
    fn another_folder_is_left_alone() {
        let other = "01BRZ3NDEKTSV4RRFFQ69G5FAV-old";
        assert_eq!(relinked(&format!("[a](_assets/{other}/a.png)")), None);
    }

    #[test]
    fn an_empty_folder_name_rewrites_nothing() {
        assert_eq!(relink_folder("[a](_assets//a.png)", "", TO), None);
    }

    #[test]
    fn a_body_without_the_folder_is_not_rewritten() {
        assert_eq!(relinked("nothing to see"), None);
    }
}
