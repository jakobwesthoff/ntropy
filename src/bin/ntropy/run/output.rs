// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Plain, aligned output (ADRs 0019, 0025, 0033).
//!
//! The non-interactive note table is `id date title tags path`, one note per
//! line led by an uppercase column header. Columns are padded to their widest
//! cell in Unicode display width and the final column is left unpadded, so the
//! header stays self-describing and `tail -n +2` strips it; tags are
//! comma-joined within their field. Scan warnings go to stderr by file name
//! (stdout stays clean for piping). The `tags` command prints `TAG COUNT` and
//! `view list` prints `NAME FIELD` through the same renderer. The same note
//! fields render as a human reference via [`note_reference`].

use std::fmt::Display;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use ntropy::config::ViewConfig;
use ntropy::note::Note;
use ntropy::ops::{TagCount, VaultStats};
use ntropy::scan::ScanWarning;
use ntropy::vault::{ResolveSource, Vault};
use unicode_width::UnicodeWidthStr;

/// Print notes as an aligned `id date title tags path` table, newest first, led
/// by a column header (ADRs 0025, 0033).
pub fn print_notes(notes: &[Note]) -> Result<()> {
    let rows = notes
        .iter()
        .map(|note| {
            Ok(vec![
                note.id.to_string(),
                note.created_date()
                    .context("while computing the note's creation date")?,
                note.title.clone(),
                note.tags.join(","),
                note.path.display().to_string(),
            ])
        })
        .collect::<Result<Vec<_>>>()?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_table(&mut out, &["ID", "DATE", "TITLE", "TAGS", "PATH"], &rows)
        .context("while printing the notes table")
}

/// A human-readable note reference: `date  title  [tags]  (id)`.
///
/// The single representation used wherever a note is named to a human (delete
/// prompts and confirmations, the ambiguous-match list). Tags are dropped when
/// empty. The `id`/`date`/`title`/`tags` are taken as primitives so both a
/// [`Note`] and a picker candidate can format identically.
pub fn reference(id: impl Display, date: &str, title: &str, tags: &[String]) -> String {
    let mut s = format!("{date}  {title}");
    if !tags.is_empty() {
        s.push_str(&format!("  [{}]", tags.join(", ")));
    }
    s.push_str(&format!("  ({id})"));
    s
}

/// The [`reference`] for a parsed [`Note`], computing its local creation date.
pub fn note_reference(note: &Note) -> Result<String> {
    Ok(reference(
        note.id,
        &note.created_date()?,
        &note.title,
        &note.tags,
    ))
}

/// Print tags as an aligned `TAG COUNT` table, led by a column header.
pub fn print_tags(tags: &[TagCount]) -> Result<()> {
    let rows: Vec<Vec<String>> = tags
        .iter()
        .map(|entry| vec![entry.tag.clone(), entry.count.to_string()])
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_table(&mut out, &["TAG", "COUNT"], &rows).context("while printing the tags table")
}

/// Print configured views as an aligned `NAME FIELD` table, led by a column
/// header.
pub fn print_views(views: &[ViewConfig]) -> Result<()> {
    let rows: Vec<Vec<String>> = views
        .iter()
        .map(|view| vec![view.name.clone(), view.field.clone()])
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_table(&mut out, &["NAME", "FIELD"], &rows).context("while printing the views table")
}

/// Print the `info` report: the active vault and how it resolved, the global
/// default, and the vault statistics. This is a human report shown identically
/// whether interactive or piped (it is not a machine table).
pub fn print_info(
    vault: &Vault,
    source: &ResolveSource,
    default_vault: Option<&Path>,
    stats: &VaultStats,
) {
    println!(
        "Active vault:  {} (via {})",
        vault.root().display(),
        describe_source(source)
    );
    match default_vault {
        Some(path) => println!("Default vault: {}", path.display()),
        None => println!("Default vault: (not set)"),
    }

    println!();
    println!("Notes:     {}", stats.notes);
    println!("Tags:      {}", stats.distinct_tags);
    println!("Views:     {}", stats.views);
    println!("Templates: {}", stats.templates.len());
    println!("Warnings:  {}", stats.warnings);
    match (&stats.oldest_date, &stats.newest_date) {
        (Some(oldest), Some(newest)) => println!("Span:      {oldest} .. {newest}"),
        _ => println!("Span:      (no notes)"),
    }

    if !stats.top_tags.is_empty() {
        println!();
        println!("Top tags:");
        let width = stats
            .top_tags
            .iter()
            .map(|t| t.tag.width())
            .max()
            .unwrap_or(0);
        for entry in &stats.top_tags {
            println!("  {}  {}", pad(&entry.tag, width), entry.count);
        }
    }

    if !stats.templates.is_empty() {
        println!();
        println!("Templates:");
        for name in &stats.templates {
            println!("  {name}");
        }
    }
}

/// A human description of which rule resolved the active vault.
fn describe_source(source: &ResolveSource) -> String {
    match source {
        ResolveSource::Explicit => "--vault flag".to_string(),
        ResolveSource::Env => "$NTROPY_VAULT".to_string(),
        ResolveSource::Pointer(path) => format!("pointer file {}", path.display()),
        ResolveSource::WalkUp => "current directory".to_string(),
        ResolveSource::GlobalDefault => "global default".to_string(),
    }
}

/// Print scan warnings to stderr, one per line, identified by file name.
///
/// Only the file name is shown (not the absolute path) so the message is stable
/// and readable; the file is always a top-level entry in `all-notes/`.
pub fn print_warnings(warnings: &[ScanWarning]) {
    for warning in warnings {
        let name = warning
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| warning.path.display().to_string());
        eprintln!("warning: skipped `{name}`: {}", warning.message);
    }
}

// =============================================================================
// Aligned table renderer (ADR 0033)
// =============================================================================

/// Render a plain table to `out`: the uppercase `header` row followed by one
/// line per row.
///
/// Every column but the last is padded with spaces to its widest cell, measured
/// in Unicode display columns so wide (CJK) and zero-width characters align;
/// columns are separated by two spaces. The final column is emitted unpadded, so
/// no line carries trailing whitespace and the `tail -n +2` header strip stays
/// valid. Each row is expected to carry exactly as many cells as the header.
fn write_table(out: &mut impl Write, header: &[&str], rows: &[Vec<String>]) -> io::Result<()> {
    let widths = column_widths(header, rows);
    write_row(out, header.iter().copied(), &widths)?;
    for row in rows {
        write_row(out, row.iter().map(String::as_str), &widths)?;
    }
    Ok(())
}

/// The display-column width of each column: the widest of the header cell and
/// every row's cell in that position.
fn column_widths(header: &[&str], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = header.iter().map(|cell| cell.width()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.width());
        }
    }
    widths
}

/// Write one row: pad every cell but the last to its column width, separate
/// columns with two spaces, and terminate with a newline.
fn write_row<'a>(
    out: &mut impl Write,
    cells: impl ExactSizeIterator<Item = &'a str>,
    widths: &[usize],
) -> io::Result<()> {
    let last = cells.len().saturating_sub(1);
    for (i, cell) in cells.enumerate() {
        if i > 0 {
            write!(out, "  ")?;
        }
        if i == last {
            write!(out, "{cell}")?;
        } else {
            write!(out, "{}", pad(cell, widths[i]))?;
        }
    }
    writeln!(out)
}

/// Right-pad `s` with spaces to `width` display columns (never truncates).
fn pad(s: &str, width: usize) -> String {
    let w = s.width();
    let mut out = s.to_string();
    if w < width {
        out.push_str(&" ".repeat(width - w));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn reference_without_tags_omits_brackets() {
        let r = reference(ULID, "2026-06-25", "My Note", &[]);
        assert_eq!(r, "2026-06-25  My Note  (01ARZ3NDEKTSV4RRFFQ69G5FAV)");
    }

    #[test]
    fn reference_with_one_tag() {
        let r = reference(ULID, "2026-06-25", "My Note", &["work".to_string()]);
        assert_eq!(
            r,
            "2026-06-25  My Note  [work]  (01ARZ3NDEKTSV4RRFFQ69G5FAV)"
        );
    }

    #[test]
    fn reference_joins_multiple_tags_with_commas() {
        let tags = ["area/work".to_string(), "home".to_string()];
        let r = reference(ULID, "2026-06-25", "My Note", &tags);
        assert_eq!(
            r,
            "2026-06-25  My Note  [area/work, home]  (01ARZ3NDEKTSV4RRFFQ69G5FAV)"
        );
    }

    #[test]
    fn note_reference_uses_note_fields() {
        let note = Note::parse(
            std::path::PathBuf::from(format!("/v/all-notes/{ULID}-n.md")),
            "---\ntitle: Quarterly\ntags: [area/work]\n---\nbody\n",
            None,
        )
        .expect("parse");
        let r = note_reference(&note).expect("reference");
        // The date is timezone-derived, so assert the stable parts only.
        assert!(r.contains("  Quarterly  [area/work]  "));
        assert!(r.ends_with(&format!("({ULID})")));
    }

    // -------------------------------------------------------------------------
    // Aligned table renderer
    // -------------------------------------------------------------------------

    use unicode_width::UnicodeWidthStr;

    /// Render a table to a `String` so its exact bytes (including padding) can be
    /// asserted without touching stdout.
    fn render(header: &[&str], rows: &[Vec<String>]) -> String {
        let mut buf = Vec::new();
        write_table(&mut buf, header, rows).expect("writing to a Vec never fails");
        String::from_utf8(buf).expect("renderer emits valid UTF-8")
    }

    /// Build an owned row from string slices.
    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    // `pad` is the unit that turns a cell into a fixed display width; the
    // `write_table` tests below build their expected output from it, so its
    // behaviour is pinned independently here first.

    #[test]
    fn pad_fills_to_display_width_with_spaces() {
        assert_eq!(pad("ab", 5), "ab   ");
        assert_eq!(pad("ab", 2), "ab");
    }

    #[test]
    fn pad_never_truncates_an_over_wide_cell() {
        assert_eq!(pad("toolong", 3), "toolong");
    }

    #[test]
    fn pad_counts_wide_characters_as_two_columns() {
        // "日" is two display columns, so reaching width 4 needs two spaces.
        assert_eq!(pad("日", 4), "日  ");
        assert_eq!(pad("日本語", 6), "日本語");
    }

    #[test]
    fn pad_counts_combining_marks_as_zero_columns() {
        // 'e' + combining acute renders in one column, so width 3 needs two
        // trailing spaces.
        let combining = "e\u{0301}";
        assert_eq!(combining.width(), 1);
        assert_eq!(pad(combining, 3), format!("{combining}  "));
    }

    #[test]
    fn equal_width_columns_use_two_space_separators() {
        // An independent oracle (no `pad` in the expectation): equal-width cells
        // pin the header-first ordering, the two-space separator, the unpadded
        // last column, and newline termination.
        let out = render(&["AA", "BB"], &[row(&["11", "22"]), row(&["33", "44"])]);
        assert_eq!(out, "AA  BB\n11  22\n33  44\n");
    }

    #[test]
    fn columns_pad_to_the_widest_cell() {
        // The first column's width is driven by the long second row; the last
        // column (COUNT) is never padded.
        let header = ["TAG", "COUNT"];
        let rows = [row(&["work", "3"]), row(&["area/very/long/path", "1"])];
        let w = "area/very/long/path".width();
        let expected = format!(
            "{}  {}\n{}  {}\n{}  {}\n",
            pad("TAG", w),
            "COUNT",
            pad("work", w),
            "3",
            pad("area/very/long/path", w),
            "1",
        );
        assert_eq!(render(&header, &rows), expected);
    }

    #[test]
    fn header_sets_the_floor_width() {
        // Every body cell is narrower than its header, so the header text governs
        // the column width and the body cell pads out to it.
        let w = "NAME".width();
        let expected = format!(
            "{}  {}\n{}  {}\n",
            pad("NAME", w),
            "FIELD",
            pad("a", w),
            "b"
        );
        assert_eq!(render(&["NAME", "FIELD"], &[row(&["a", "b"])]), expected);
    }

    #[test]
    fn header_only_table_still_prints_the_header() {
        // An empty result (e.g. `tags` on an empty vault) prints just the header.
        assert_eq!(render(&["TAG", "COUNT"], &[]), "TAG  COUNT\n");
    }

    #[test]
    fn empty_middle_cell_is_padded_to_its_column() {
        // A note with no tags still occupies the full TAGS column so the PATH
        // column stays aligned across rows.
        let header = ["TITLE", "TAGS", "PATH"];
        let rows = [row(&["Alpha", "work", "a.md"]), row(&["Beta", "", "b.md"])];
        let tw = "TITLE".width().max("Alpha".width());
        let gw = "TAGS".width();
        let expected = format!(
            "{}  {}  {}\n{}  {}  {}\n{}  {}  {}\n",
            pad("TITLE", tw),
            pad("TAGS", gw),
            "PATH",
            pad("Alpha", tw),
            pad("work", gw),
            "a.md",
            pad("Beta", tw),
            pad("", gw),
            "b.md",
        );
        assert_eq!(render(&header, &rows), expected);
    }

    #[test]
    fn single_column_table_is_unpadded() {
        // With one column it is also the last column, so nothing is padded.
        let out = render(&["NAME"], &[row(&["short"]), row(&["a-much-longer-name"])]);
        assert_eq!(out, "NAME\nshort\na-much-longer-name\n");
    }

    #[test]
    fn no_line_carries_trailing_whitespace() {
        // The final column is unpadded, so even with ragged middle columns and an
        // empty trailing cell no line ends in a space.
        let out = render(
            &["ID", "TAGS", "PATH"],
            &[row(&["01", "work,home", "a.md"]), row(&["02", "", "b.md"])],
        );
        for line in out.lines() {
            assert!(
                !line.ends_with(' '),
                "line must not end with whitespace: {line:?}"
            );
        }
    }

    #[test]
    fn wide_characters_align_by_display_width() {
        // A CJK title occupies two display columns per character, so the column
        // width is computed from display width, not byte length. If width were
        // byte-based, "日本語" (9 bytes) would over-pad relative to "ascii".
        let header = ["TITLE", "PATH"];
        let rows = [row(&["日本語", "wide.md"]), row(&["ascii", "narrow.md"])];
        let w = "日本語".width();
        assert_eq!(("日本語".len(), w), (9, 6));
        let expected = format!(
            "{}  {}\n{}  {}\n{}  {}\n",
            pad("TITLE", w),
            "PATH",
            pad("日本語", w),
            "wide.md",
            pad("ascii", w),
            "narrow.md",
        );
        assert_eq!(render(&header, &rows), expected);
    }
}
