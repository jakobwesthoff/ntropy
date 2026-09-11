// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The navigation fragments a page carries: the sidebar, the outline, note
//! listings, related notes, and the generated front page (ADR 0054). Each is
//! assembled here from the [`Model`] as an HTML string the page template
//! splices in.
//!
//! Every `href` is relative to the page being rendered, through the page's
//! root prefix (`../` per directory level), so the site works from `file://`.

use crate::render::html::writer::escape;
use crate::render::markdown::Heading;

use super::model::{Entry, Group, Model, NavItem};

/// The tag glyph before a tag link, from the page's icon sprite.
const TAG_ICON: &str = "<svg class=\"icon\" aria-hidden=\"true\"><use href=\"#icon-tag\"/></svg>";
/// The disclosure chevron of a collapsible summary; the stylesheet turns
/// it when the details are open.
const CHEVRON: &str =
    "<svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg>";

/// The `../` prefix that reaches the site root from `page`, a site-relative
/// path such as `tags/a/index.html`.
pub fn prefix_for(page: &str) -> String {
    "../".repeat(page.matches('/').count())
}

/// The sidebar (ADR 0054, ADR 0056): the model's sidebar sections, each
/// either a cloud of groups with counts (the tag section) or a collapsible
/// tree of items. A tree section starts open only when it holds the
/// current page, and so does every group on the page's trail, so a reader
/// lands with the neighbourhood of the page in view and nothing else
/// unfolded.
pub fn sidebar(model: &Model, prefix: &str, current: &str) -> String {
    let mut out = String::from("<nav class=\"sidebar\">\n");
    for section in &model.sidebar {
        let label = escape(&section.label);
        if section.cloud {
            out.push_str("<section class=\"nav-section nav-tags\">\n");
            match &section.page {
                Some((page, _)) => out.push_str(&format!(
                    "<h2 class=\"nav-title\"><a href=\"{}\">{label}</a></h2>\n",
                    escape(&format!("{prefix}{page}"))
                )),
                None => out.push_str(&format!("<h2 class=\"nav-title\">{label}</h2>\n")),
            }
            tag_cloud(&mut out, model, &section.items, prefix, current);
            out.push_str("</section>\n");
            continue;
        }
        let open = if section
            .items
            .iter()
            .any(|item| contains_item(item, model, current))
            || section
                .page
                .as_ref()
                .is_some_and(|(page, _)| page == current)
        {
            " open"
        } else {
            ""
        };
        out.push_str(&format!(
            "<details class=\"nav-section\"{open}>\n<summary class=\"nav-title\">{CHEVRON}{label}</summary>\n"
        ));
        if let Some((page, text)) = &section.page {
            out.push_str(&format!(
                "<a class=\"nav-all\" href=\"{}\">{text}</a>\n",
                escape(&format!("{prefix}{page}"))
            ));
        }
        nav_items(&mut out, model, &section.items, prefix, current);
        out.push_str("</details>\n");
    }
    out.push_str("</nav>\n");
    out
}

/// A list of nav items: notes, groups with their subtrees, and
/// hand-assembled groups.
fn nav_items(out: &mut String, model: &Model, items: &[NavItem], prefix: &str, current: &str) {
    if items.is_empty() {
        return;
    }
    out.push_str("<ul class=\"nav-entries\">\n");
    for item in items {
        match item {
            NavItem::Note { index, label } => {
                let note = &model.notes[*index];
                note_item(
                    out,
                    note.page.as_str(),
                    label.as_deref().unwrap_or(note.name()),
                    prefix,
                    current,
                );
            }
            NavItem::Group {
                section,
                path,
                label,
            } => {
                let group = model.group_at(*section, path);
                group_item(out, model, group, label.as_deref(), prefix, current);
            }
            NavItem::Curated {
                label,
                items: inner,
            } => {
                let open = if inner.iter().any(|item| contains_item(item, model, current)) {
                    " open"
                } else {
                    ""
                };
                out.push_str(&format!(
                    "<li class=\"nav-group\"><details{open}><summary>{CHEVRON}<span>{}</span></summary>\n",
                    escape(label)
                ));
                nav_items(out, model, inner, prefix, current);
                out.push_str("</details></li>\n");
            }
        }
    }
    out.push_str("</ul>\n");
}

/// One note as a list item, marked current on its own page.
fn note_item(out: &mut String, page: &str, name: &str, prefix: &str, current: &str) {
    let current_attr = if page == current {
        " aria-current=\"page\""
    } else {
        ""
    };
    out.push_str(&format!(
        "<li class=\"nav-note\"><a href=\"{}\"{current_attr}>{}</a></li>\n",
        escape(&format!("{prefix}{page}")),
        escape(name)
    ));
}

/// One group as a collapsible item: its label (or the override) linking to
/// its page, marked current when the page is the group's own, as a landing
/// note's is, then its entries, notes and child groups interleaved in
/// reading order.
fn group_item(
    out: &mut String,
    model: &Model,
    group: &Group,
    label: Option<&str>,
    prefix: &str,
    current: &str,
) {
    let open = if contains_page(group, model, current) {
        " open"
    } else {
        ""
    };
    let current_attr = if group.page == current {
        " aria-current=\"page\""
    } else {
        ""
    };
    out.push_str(&format!(
        "<li class=\"nav-group\"><details{open}><summary>{CHEVRON}<a href=\"{}\"{current_attr}>{}</a></summary>\n",
        escape(&format!("{prefix}{}", group.page)),
        escape(label.unwrap_or(&group.label))
    ));
    if !group.entries.is_empty() {
        out.push_str("<ul class=\"nav-entries\">\n");
        for entry in &group.entries {
            match *entry {
                Entry::Note(index) => {
                    let note = &model.notes[index];
                    note_item(out, &note.page, note.name(), prefix, current);
                }
                Entry::Child(child) => {
                    group_item(out, model, &group.children[child], None, prefix, current);
                }
            }
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</details></li>\n");
}

/// The top-level tags as a wrapped list of links with counts; the one whose
/// tree holds the current page is marked current.
fn tag_cloud(out: &mut String, model: &Model, items: &[NavItem], prefix: &str, current: &str) {
    let groups: Vec<(&Group, Option<&str>)> = items
        .iter()
        .filter_map(|item| match item {
            NavItem::Group {
                section,
                path,
                label,
            } => Some((model.group_at(*section, path), label.as_deref())),
            _ => None,
        })
        .collect();
    if groups.is_empty() {
        return;
    }
    out.push_str("<ul class=\"tag-cloud\">\n");
    for (group, label) in groups {
        let current_attr = if contains_page(group, model, current) {
            " aria-current=\"true\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<li><a href=\"{}\"{current_attr}>{}<span class=\"count\">{}</span></a></li>\n",
            escape(&format!("{prefix}{}", group.page)),
            escape(label.unwrap_or(&group.label)),
            group.descendants().len()
        ));
    }
    out.push_str("</ul>\n");
}

/// Whether the current page lies under a nav item.
fn contains_item(item: &NavItem, model: &Model, current: &str) -> bool {
    match item {
        NavItem::Note { index, .. } => model.notes[*index].page == current,
        NavItem::Group { section, path, .. } => {
            contains_page(model.group_at(*section, path), model, current)
        }
        NavItem::Curated { items, .. } => {
            items.iter().any(|item| contains_item(item, model, current))
        }
    }
}

/// Whether the current page is this group's page (a landing note's page is
/// that), one of its notes, or anything below it.
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
    let mut out = String::from("<nav class=\"outline\" aria-label=\"On this page\">\n<ul>\n");
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

/// A list of notes, one row each: creation date, then the title linking to
/// the page with the tags as links following it.
pub fn listing(model: &Model, notes: &[usize], prefix: &str) -> String {
    if notes.is_empty() {
        return String::from("<p class=\"empty\">No notes.</p>\n");
    }
    let mut out = String::from("<ol class=\"note-rows\">\n");
    for &index in notes {
        let note = &model.notes[index];
        out.push_str(&format!(
            "<li class=\"note-row\"><time datetime=\"{date}\">{date}</time><span class=\"note-row-main\"><a class=\"note-row-title\" href=\"{href}\">{title}</a>",
            date = escape(&note.created),
            href = escape(&format!("{prefix}{}", note.page)),
            title = escape(&note.title)
        ));
        if !note.tags.is_empty() {
            out.push_str("<span class=\"note-row-tags\">");
            for tag in &note.tags {
                out.push_str(&format!(
                    "<a class=\"tag\" href=\"{}\">{TAG_ICON}{}</a>",
                    escape(&format!("{prefix}tags/{tag}/index.html")),
                    escape(tag)
                ));
            }
            out.push_str("</span>");
        }
        out.push_str("</span></li>\n");
    }
    out.push_str("</ol>\n");
    out
}

/// The child groups of a section or a group as chips, each with the number
/// of notes under it.
pub fn group_list(groups: &[Group], prefix: &str) -> String {
    if groups.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul class=\"group-chips\">\n");
    for group in groups {
        out.push_str(&format!(
            "<li><a class=\"chip\" href=\"{}\">{}<span class=\"count\">{}</span></a></li>\n",
            escape(&format!("{prefix}{}", group.page)),
            escape(&group.label),
            group.descendants().len()
        ));
    }
    out.push_str("</ul>\n");
    out
}

/// The notes related to a note page, as a listing under its own heading;
/// empty when there are none.
pub fn related(model: &Model, notes: &[usize], prefix: &str) -> String {
    if notes.is_empty() {
        return String::new();
    }
    format!(
        "<section class=\"related\">\n<h2>Related notes</h2>\n{}</section>\n",
        listing(model, notes, prefix)
    )
}

/// The generated front page: the note count and the span of dates, the
/// newest visible notes, and each section's top-level groups.
pub fn overview(model: &Model, prefix: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<header class=\"front\">\n<h1 class=\"site-title\">{}</h1>\n",
        escape(&model.title)
    ));
    let count = model.notes.len();
    match (model.notes.first(), model.notes.last()) {
        (Some(newest), Some(oldest)) if count > 1 => out.push_str(&format!(
            "<p class=\"note-count\">{count} notes, <time datetime=\"{oldest}\">{oldest}</time> to <time datetime=\"{newest}\">{newest}</time></p>\n",
            oldest = escape(&oldest.created),
            newest = escape(&newest.created)
        )),
        (Some(only), _) => out.push_str(&format!(
            "<p class=\"note-count\">1 note, <time datetime=\"{date}\">{date}</time></p>\n",
            date = escape(&only.created)
        )),
        _ => out.push_str("<p class=\"note-count\">No notes.</p>\n"),
    }
    out.push_str("</header>\n");

    let recent = model.recent();
    if !recent.is_empty() {
        out.push_str("<section class=\"recent\">\n<h2>Newest notes</h2>\n");
        out.push_str(&listing(model, &recent, prefix));
        out.push_str("</section>\n");
    }

    for section in &model.sections {
        if section.groups.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "<section class=\"section-summary\">\n<h2><a href=\"{}\">{}</a></h2>\n",
            escape(&format!("{prefix}{}", section.page)),
            escape(&section.title)
        ));
        out.push_str(&group_list(&section.groups, prefix));
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
        // The section and the `done` group hold the current note and open;
        // `open` stays closed.
        assert!(
            out.contains("<details class=\"nav-section\" open>\n<summary class=\"nav-title\"><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg>by-status</summary>"),
            "{out}"
        );
        assert!(
            out.contains(
                "<details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../views/by-status/done/index.html\">done</a>"
            ),
            "{out}"
        );
        assert!(
            out.contains(
                "<details><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../views/by-status/open/index.html\">open</a>"
            ),
            "{out}"
        );
        // The tag cloud marks the top-level tag holding the note, with the
        // count of notes under it.
        assert!(
            out.contains(
                "<li><a href=\"../tags/programming/index.html\" aria-current=\"true\">programming<span class=\"count\">1</span></a></li>"
            ),
            "{out}"
        );
    }

    #[test]
    fn sidebar_sections_stay_closed_away_from_their_pages() {
        let out = sidebar(&model(), "", "index.html");
        assert!(
            out.contains("<details class=\"nav-section\">\n<summary class=\"nav-title\"><svg class=\"icon chevron\""),
            "{out}"
        );
        assert!(!out.contains("aria-current"), "{out}");
        // A section's own page opens it.
        let own = sidebar(&model(), "../../", "views/by-status/index.html");
        assert!(
            own.contains("<details class=\"nav-section\" open>"),
            "{own}"
        );
    }

    #[test]
    fn sidebar_opens_the_group_of_a_group_page_and_marks_its_link() {
        let out = sidebar(&model(), "../../../", "views/by-status/done/index.html");
        assert!(
            out.contains("<details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../../../views/by-status/done/index.html\" aria-current=\"page\">done</a>"),
            "{out}"
        );
    }

    #[test]
    fn sidebar_shows_labels_and_marks_a_landing_page_current() {
        const C: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";
        let notes = [
            note(
                A,
                "Basics",
                "area: docs/start\nsite:\n  index: true\n  label: Getting Started\n",
            ),
            note(
                B,
                "Installation Guide",
                "area: docs/start\nsite:\n  label: Install\n  order: 1\n",
            ),
            note(C, "Later", "area: docs/start\n"),
        ];
        let model = Model::build(
            &notes,
            &[ViewDef::new("by-area", "area")],
            &SiteOptions::default(),
            "V",
        )
        .expect("model");
        // Seen from the landing page: the group link is the current page,
        // the landing note is no entry, the labelled note shows its label,
        // ordered before the newer unordered one.
        let out = sidebar(
            &model,
            "../../../../",
            "views/by-area/docs/start/index.html",
        );
        assert!(
            out.contains("<a href=\"../../../../views/by-area/docs/start/index.html\" aria-current=\"page\">Getting Started</a></summary>\n<ul class=\"nav-entries\">\n<li class=\"nav-note\"><a href=\"../../../../notes/installation-guide.html\">Install</a></li>\n<li class=\"nav-note\"><a href=\"../../../../notes/later.html\">Later</a></li>\n</ul>"),
            "{out}"
        );
        assert!(!out.contains(">Basics<"), "{out}");
        // The parent group opens too, since the page lies below it.
        assert!(
            out.contains("<li class=\"nav-group\"><details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../../../../views/by-area/docs/index.html\">docs</a>"),
            "{out}"
        );
    }

    fn docs_model(options: SiteOptions) -> Model {
        const C: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";
        let notes = [
            note(
                A,
                "Basics",
                "tags: [docs/start]\nsite:\n  index: true\n  label: Getting Started\n",
            ),
            note(B, "Install", "tags: [docs/start]\n"),
            note(C, "Aside", "tags: [misc]\nstatus: open\n"),
        ];
        Model::build(
            &notes,
            &[ViewDef::new("by-status", "status")],
            &options,
            "V",
        )
        .expect("model")
    }

    #[test]
    fn a_rooted_sidebar_is_one_tree_with_an_overview_link() {
        let model = docs_model(SiteOptions {
            root: Some("tags/docs".to_string()),
            ..SiteOptions::default()
        });
        let out = sidebar(&model, "../", "notes/install.html");
        assert_eq!(
            out,
            "<nav class=\"sidebar\">\n<details class=\"nav-section\" open>\n<summary class=\"nav-title\"><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg>docs</summary>\n<a class=\"nav-all\" href=\"../tags/docs/index.html\">Overview</a>\n<ul class=\"nav-entries\">\n<li class=\"nav-group\"><details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/start/index.html\">Getting Started</a></summary>\n<ul class=\"nav-entries\">\n<li class=\"nav-note\"><a href=\"../notes/install.html\" aria-current=\"page\">Install</a></li>\n</ul>\n</details></li>\n</ul>\n</details>\n</nav>\n"
        );
        // The root's own page opens the section without marking an item.
        let own = sidebar(&model, "../../", "tags/docs/index.html");
        assert!(
            own.starts_with("<nav class=\"sidebar\">\n<details class=\"nav-section\" open>"),
            "{own}"
        );
        assert!(!own.contains("aria-current"), "{own}");
    }

    #[test]
    fn a_curated_sidebar_shows_labels_and_hand_made_groups_without_links() {
        let model = docs_model(
            toml::from_str(&format!(
                "[[nav]]\nlabel = \"Guide\"\nitems = [\n  {{ note = \"{B}\", label = \"Setting up\" }},\n  {{ label = \"Reference\", items = [{{ label = \"Start\", tag = \"docs/start\" }}, {{ tags = true }}] }},\n]\n"
            ))
            .expect("nav parses"),
        );
        let out = sidebar(&model, "../", "notes/install.html");
        // The section has no page, so no link under its title; the note
        // shows its nav label and is current; the hand-made group is a
        // summary without a link, open because the page lies below it.
        assert!(
            out.starts_with("<nav class=\"sidebar\">\n<details class=\"nav-section\" open>\n<summary class=\"nav-title\"><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg>Guide</summary>\n<ul class=\"nav-entries\">\n<li class=\"nav-note\"><a href=\"../notes/install.html\" aria-current=\"page\">Setting up</a></li>\n<li class=\"nav-group\"><details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><span>Reference</span></summary>\n<ul class=\"nav-entries\">\n<li class=\"nav-group\"><details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/start/index.html\">Start</a></summary>"),
            "{out}"
        );
        assert!(!out.contains("nav-all"), "{out}");
        // The whole tag tree as a hand-made group holds the top-level tags
        // as groups; it opens too, since the page lies under `docs`, while
        // `misc` stays closed.
        assert!(
            out.contains("<li class=\"nav-group\"><details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><span>tags</span></summary>\n<ul class=\"nav-entries\">\n<li class=\"nav-group\"><details open><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/docs/index.html\">docs</a>"),
            "{out}"
        );
        assert!(
            out.contains("<li class=\"nav-group\"><details><summary><svg class=\"icon chevron\" aria-hidden=\"true\"><use href=\"#icon-chevron-right\"/></svg><a href=\"../tags/misc/index.html\">misc</a>"),
            "{out}"
        );
    }

    #[test]
    fn overview_lists_visible_notes_only() {
        let notes = [
            note(A, "Shown", "tags: [t]\n"),
            note(B, "Hidden", "tags: [t]\nsite:\n  hidden: true\n"),
        ];
        let model = Model::build(&notes, &[], &SiteOptions::default(), "V").expect("model");
        let out = overview(&model, "");
        assert!(out.contains("2 notes"), "{out}");
        assert!(out.contains("notes/shown.html"), "{out}");
        assert!(!out.contains("notes/hidden.html"), "{out}");
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
            "<nav class=\"outline\" aria-label=\"On this page\">\n<ul>\n<li class=\"depth-2\"><a href=\"#intro\">Intro &amp; more</a></li>\n<li class=\"depth-3\"><a href=\"#details\">Details</a></li>\n</ul>\n</nav>\n"
        );
        assert_eq!(outline(&[]), "");
    }

    #[test]
    fn listing_rows_show_date_title_and_tag_links() {
        let model = model();
        let out = listing(&model, &[1], "../");
        assert_eq!(
            out,
            "<ol class=\"note-rows\">\n<li class=\"note-row\"><time datetime=\"2016-07-31\">2016-07-31</time><span class=\"note-row-main\"><a class=\"note-row-title\" href=\"../notes/rust-tips.html\">Rust &lt;Tips&gt;</a><span class=\"note-row-tags\"><a class=\"tag\" href=\"../tags/programming/rust/index.html\"><svg class=\"icon\" aria-hidden=\"true\"><use href=\"#icon-tag\"/></svg>programming/rust</a></span></span></li>\n</ol>\n"
        );
        // A note without tags has no tag span at all.
        let plain = listing(&model, &[0], "");
        assert!(!plain.contains("note-row-tags"), "{plain}");
        assert_eq!(
            listing(&model, &[], ""),
            "<p class=\"empty\">No notes.</p>\n"
        );
    }

    #[test]
    fn group_chips_count_descendants() {
        let model = model();
        let tags = &model.sections[1];
        let out = group_list(&tags.groups, "");
        assert_eq!(
            out,
            "<ul class=\"group-chips\">\n<li><a class=\"chip\" href=\"tags/programming/index.html\">programming<span class=\"count\">1</span></a></li>\n</ul>\n"
        );
        assert_eq!(group_list(&[], ""), "");
    }

    #[test]
    fn related_wraps_a_listing_or_is_empty() {
        let model = model();
        assert_eq!(related(&model, &[], "../"), "");
        let out = related(&model, &[0], "../");
        assert!(
            out.starts_with(
                "<section class=\"related\">\n<h2>Related notes</h2>\n<ol class=\"note-rows\">"
            ),
            "{out}"
        );
        assert!(out.ends_with("</section>\n"), "{out}");
    }

    #[test]
    fn overview_pins_its_structure() {
        insta::assert_snapshot!(overview(&model(), ""));
    }

    #[test]
    fn overview_counts_one_note_and_none() {
        let one = [note(A, "Only", "")];
        let model = Model::build(&one, &[], &SiteOptions::default(), "V").expect("model");
        let out = overview(&model, "");
        assert!(
            out.contains("<p class=\"note-count\">1 note, <time datetime=\"2016-07-31\">2016-07-31</time></p>"),
            "{out}"
        );
        let none = Model::build(&[], &[], &SiteOptions::default(), "V").expect("model");
        let out = overview(&none, "");
        assert!(
            out.contains("<p class=\"note-count\">No notes.</p>"),
            "{out}"
        );
        assert!(!out.contains("Newest notes"), "{out}");
    }
}
