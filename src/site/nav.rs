// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The navigation fragments a page carries: the sidebar, the outline, note
//! listings, and the generated front page (ADR 0054). Each is assembled here
//! from the [`Model`] as an HTML string the page template splices in.
//!
//! Every `href` is relative to the page being rendered, through the page's
//! root prefix (`../` per directory level), so the site works from `file://`.

use crate::render::html::writer::escape;
use crate::render::markdown::Heading;

use super::model::{FRONT_PAGE_RECENT, Group, Model};

/// The `../` prefix that reaches the site root from `page`, a site-relative
/// path such as `tags/a/index.html`.
pub fn prefix_for(page: &str) -> String {
    "../".repeat(page.matches('/').count())
}

/// The sidebar: one section per configured view and one for the tags, each
/// a tree of collapsible groups holding their notes. The groups on the
/// current page's trail start open, everything else closed, so a reader
/// lands with the neighbourhood of the page in view.
pub fn sidebar(model: &Model, prefix: &str, current: &str) -> String {
    let mut out = String::from("<nav class=\"sidebar\">\n");
    for section in &model.sections {
        out.push_str("<section class=\"nav-section\">\n");
        out.push_str(&format!(
            "<h2 class=\"nav-title\"><a href=\"{}\">{}</a></h2>\n",
            escape(&format!("{prefix}{}", section.page)),
            escape(&section.title)
        ));
        groups(&mut out, model, &section.groups, prefix, current);
        out.push_str("</section>\n");
    }
    out.push_str("</nav>\n");
    out
}

fn groups(out: &mut String, model: &Model, groups: &[Group], prefix: &str, current: &str) {
    if groups.is_empty() {
        return;
    }
    out.push_str("<ul class=\"nav-groups\">\n");
    for group in groups {
        let open = if contains_page(group, model, current) {
            " open"
        } else {
            ""
        };
        out.push_str(&format!(
            "<li class=\"nav-group\"><details{open}><summary><a href=\"{}\">{}</a></summary>\n",
            escape(&format!("{prefix}{}", group.page)),
            escape(&group.label)
        ));
        if !group.notes.is_empty() {
            out.push_str("<ul class=\"nav-notes\">\n");
            for &index in &group.notes {
                let note = &model.notes[index];
                let current_attr = if note.page == current {
                    " aria-current=\"page\""
                } else {
                    ""
                };
                out.push_str(&format!(
                    "<li><a href=\"{}\"{current_attr}>{}</a></li>\n",
                    escape(&format!("{prefix}{}", note.page)),
                    escape(&note.title)
                ));
            }
            out.push_str("</ul>\n");
        }
        self::groups(out, model, &group.children, prefix, current);
        out.push_str("</details></li>\n");
    }
    out.push_str("</ul>\n");
}

/// Whether the current page is this group's page, one of its notes, or
/// anything below it.
fn contains_page(group: &Group, model: &Model, current: &str) -> bool {
    group.page == current
        || group
            .notes
            .iter()
            .any(|&index| model.notes[index].page == current)
        || group
            .children
            .iter()
            .any(|child| contains_page(child, model, current))
}

/// The outline of a note page: its headings in order, each linking to its
/// anchor, with the level as a class for indentation. Empty when the note
/// has no headings.
pub fn outline(headings: &[Heading]) -> String {
    if headings.is_empty() {
        return String::new();
    }
    let mut out = String::from("<nav class=\"outline\">\n<ul>\n");
    for heading in headings {
        out.push_str(&format!(
            "<li class=\"depth-{}\"><a href=\"#{}\">{}</a></li>\n",
            heading.level as usize,
            escape(&heading.id),
            escape(&heading.text)
        ));
    }
    out.push_str("</ul>\n</nav>\n");
    out
}

/// A list of notes: creation date, title linking to the page, tag chips.
pub fn listing(model: &Model, notes: &[usize], prefix: &str) -> String {
    if notes.is_empty() {
        return String::from("<p class=\"empty\">No notes.</p>\n");
    }
    let mut out = String::from("<ul class=\"note-list\">\n");
    for &index in notes {
        let note = &model.notes[index];
        out.push_str(&format!(
            "<li><time datetime=\"{date}\">{date}</time> <a href=\"{href}\">{title}</a>",
            date = escape(&note.created),
            href = escape(&format!("{prefix}{}", note.page)),
            title = escape(&note.title)
        ));
        if !note.tags.is_empty() {
            out.push_str(" <span class=\"tags\">");
            for tag in &note.tags {
                out.push_str(&format!(
                    "<a class=\"tag\" href=\"{}\">{}</a> ",
                    escape(&format!("{prefix}tags/{tag}/index.html")),
                    escape(tag)
                ));
            }
            out.push_str("</span>");
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n");
    out
}

/// The child groups of a section or a group, each with the number of notes
/// under it.
pub fn group_list(model: &Model, groups: &[Group], prefix: &str) -> String {
    if groups.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul class=\"group-list\">\n");
    for group in groups {
        out.push_str(&format!(
            "<li><a href=\"{}\">{}</a> <span class=\"count\">{}</span></li>\n",
            escape(&format!("{prefix}{}", group.page)),
            escape(&group.label),
            group.descendants(model).len()
        ));
    }
    out.push_str("</ul>\n");
    out
}

/// The generated front page: the note count, the newest notes, the
/// top-level tags with their counts, and the configured views.
pub fn overview(model: &Model, prefix: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<h1 class=\"site-title\">{}</h1>\n<p class=\"note-count\">{} notes</p>\n",
        escape(&model.title),
        model.notes.len()
    ));

    let recent: Vec<usize> = (0..model.notes.len().min(FRONT_PAGE_RECENT)).collect();
    out.push_str("<section class=\"recent\">\n<h2>Newest notes</h2>\n");
    out.push_str(&listing(model, &recent, prefix));
    out.push_str("</section>\n");

    for section in &model.sections {
        if section.groups.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "<section class=\"section-summary\">\n<h2><a href=\"{}\">{}</a></h2>\n",
            escape(&format!("{prefix}{}", section.page)),
            escape(&section.title)
        ));
        out.push_str(&group_list(model, &section.groups, prefix));
        out.push_str("</section>\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Note;
    use crate::render::markdown::HeadingLevel;
    use crate::site::SiteOptions;
    use crate::text::slug;
    use crate::view::ViewDef;

    use std::path::PathBuf;

    const A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    fn note(id: &str, title: &str, frontmatter: &str) -> Note {
        let content = format!("---\ntitle: {title}\n{frontmatter}---\nBody.\n");
        Note::parse(
            PathBuf::from(format!("/vault/all-notes/{id}-{}.md", slug::slugify(title))),
            &content,
            None,
        )
        .expect("the fixture note parses")
    }

    fn model() -> Model {
        let notes = [
            note(A, "Rust <Tips>", "tags: [programming/rust]\nstatus: done\n"),
            note(B, "Plain", "status: open\n"),
        ];
        Model::build(
            &notes,
            &[ViewDef::new("by-status", "status")],
            &SiteOptions::default(),
            "Vault & Co",
        )
        .expect("model")
    }

    #[test]
    fn prefixes_climb_one_level_per_directory() {
        assert_eq!(prefix_for("index.html"), "");
        assert_eq!(prefix_for("notes/x.html"), "../");
        assert_eq!(prefix_for("tags/a/b/index.html"), "../../../");
    }

    #[test]
    fn sidebar_pins_its_structure_and_opens_the_current_trail() {
        let model = model();
        insta::assert_snapshot!(sidebar(&model, "../", "notes/rust-tips.html"));
    }

    #[test]
    fn sidebar_marks_the_current_note_and_escapes_titles() {
        let out = sidebar(&model(), "../", "notes/rust-tips.html");
        assert!(
            out.contains("aria-current=\"page\">Rust &lt;Tips&gt;</a>"),
            "{out}"
        );
        // The `done` group holds the current note and opens; `open` stays
        // closed.
        assert!(
            out.contains(
                "<details open><summary><a href=\"../views/by-status/done/index.html\">done</a>"
            ),
            "{out}"
        );
        assert!(
            out.contains(
                "<details><summary><a href=\"../views/by-status/open/index.html\">open</a>"
            ),
            "{out}"
        );
    }

    #[test]
    fn sidebar_opens_the_group_of_a_group_page() {
        let out = sidebar(&model(), "../../../", "tags/programming/rust/index.html");
        assert!(out.contains("<details open><summary><a href=\"../../../tags/programming/index.html\">programming</a>"), "{out}");
    }

    #[test]
    fn outline_lists_headings_with_their_depth() {
        let headings = [
            Heading {
                level: HeadingLevel::H2,
                id: "intro".to_string(),
                text: "Intro & more".to_string(),
            },
            Heading {
                level: HeadingLevel::H3,
                id: "details".to_string(),
                text: "Details".to_string(),
            },
        ];
        assert_eq!(
            outline(&headings),
            "<nav class=\"outline\">\n<ul>\n<li class=\"depth-2\"><a href=\"#intro\">Intro &amp; more</a></li>\n<li class=\"depth-3\"><a href=\"#details\">Details</a></li>\n</ul>\n</nav>\n"
        );
        assert_eq!(outline(&[]), "");
    }

    #[test]
    fn listing_shows_date_title_and_tag_links() {
        let model = model();
        let out = listing(&model, &[1], "../");
        assert!(
            out.contains("<a href=\"../notes/rust-tips.html\">Rust &lt;Tips&gt;</a>"),
            "{out}"
        );
        assert!(
            out.contains(
                "<a class=\"tag\" href=\"../tags/programming/rust/index.html\">programming/rust</a>"
            ),
            "{out}"
        );
        assert_eq!(
            listing(&model, &[], ""),
            "<p class=\"empty\">No notes.</p>\n"
        );
    }

    #[test]
    fn group_list_counts_descendants() {
        let model = model();
        let tags = &model.sections[1];
        let out = group_list(&model, &tags.groups, "");
        assert!(out.contains("<a href=\"tags/programming/index.html\">programming</a> <span class=\"count\">1</span>"), "{out}");
    }

    #[test]
    fn overview_pins_its_structure() {
        insta::assert_snapshot!(overview(&model(), ""));
    }
}
