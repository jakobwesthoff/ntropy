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

use std::borrow::Cow;
use std::fmt::Display;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use ntropy::config::ViewConfig;
use ntropy::note::Note;
use ntropy::ops::{TagCount, VaultStats};
use ntropy::reconcile::{Realignment, Reassignment, Rename};
use ntropy::scan::ScanWarning;
use ntropy::text::escape_controls;
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
///
/// The reference is escaped as a whole (ADR 0071). Only the title can carry
/// a character that needs it: the id and date are ntropy's own, and tags are
/// normalized slugs.
pub fn reference(id: impl Display, date: &str, title: &str, tags: &[String]) -> String {
    let mut s = format!("{date}  {title}");
    if !tags.is_empty() {
        s.push_str(&format!("  [{}]", tags.join(", ")));
    }
    s.push_str(&format!("  ({id})"));
    escape_controls(&s).into_owned()
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
        shown_path(vault.root()),
        describe_source(source)
    );
    match default_vault {
        Some(path) => println!("Default vault: {}", shown_path(path)),
        None => println!("Default vault: (not set)"),
    }

    if stats.encrypted {
        let state = if stats.unlocked { "unlocked" } else { "locked" };
        println!("Encryption:    on ({state})");
    }

    println!();
    // A locked vault has no note statistics to report, and printing zeroes
    // would make it indistinguishable from an empty vault. The rest of the
    // report needs no key, so it still prints.
    if stats.encrypted && !stats.unlocked {
        println!("Notes:     (locked; run `ntropy unlock` for note statistics)");
        println!("Views:     {}", stats.views);
        println!("Templates: {}", stats.templates.len());
    } else {
        println!("Notes:     {}", stats.notes);
        println!("Tags:      {}", stats.distinct_tags);
        println!("Views:     {}", stats.views);
        println!("Templates: {}", stats.templates.len());
        println!("Warnings:  {}", stats.warnings);
        match (&stats.oldest_date, &stats.newest_date) {
            (Some(oldest), Some(newest)) => println!("Span:      {oldest} .. {newest}"),
            _ => println!("Span:      (no notes)"),
        }
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
            println!("  {}", escape_controls(name));
        }
    }
}

/// A human description of which rule resolved the active vault.
fn describe_source(source: &ResolveSource) -> String {
    match source {
        ResolveSource::Explicit => "--vault flag".to_string(),
        ResolveSource::Env => "$NTROPY_VAULT".to_string(),
        ResolveSource::Pointer(path) => format!("pointer file {}", shown_path(path)),
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
        warn(&format_warning(warning));
    }
}

/// The text of one scan warning, without the `warning: ` prefix, escaped
/// (ADR 0071): the message can quote file names too, such as the note that
/// keeps a copy's ULID.
fn format_warning(warning: &ScanWarning) -> String {
    let message = format!(
        "skipped `{}`: {}",
        file_name(&warning.path),
        warning.message
    );
    escape_controls(&message).into_owned()
}

/// Print `message` as a warning on stderr, escaped (ADR 0071), since warnings
/// quote file names, link targets and other text from the vault.
pub fn warn(message: &str) {
    eprintln!("warning: {}", escape_controls(message));
}

/// The text `main` prints after `error: ` for an error that ends the command:
/// the whole context chain on one line, escaped (ADR 0071).
///
/// Each line is escaped on its own and the line breaks are kept, so a
/// message that spans lines, such as an external tool's diagnostics, keeps
/// them.
pub fn error_message(error: &anyhow::Error) -> String {
    format!("{error:#}")
        .split('\n')
        .map(escape_controls)
        .collect::<Vec<_>>()
        .join("\n")
}

/// `path`, when it can be printed for a script exactly as it is (ADR 0071).
///
/// A path that holds a character [`escape_controls`] would escape is refused
/// rather than printed: escaped, it would name a file that does not exist,
/// and raw, it would reach the terminal. `note` says that the path is a
/// note's, whose file name `ntropy reconcile` can replace with its canonical
/// one, so the error says to run it.
pub fn checked_path(path: &Path, note: bool) -> Result<&Path> {
    if let Cow::Owned(escaped) = escape_controls(&path.to_string_lossy()) {
        let hint = if note {
            "; run `ntropy reconcile` to rename the note"
        } else {
            ""
        };
        bail!("`{escaped}` holds control or bidi characters, so it is not printed{hint}");
    }
    Ok(path)
}

/// A path for a line meant for a human, escaped (ADR 0071).
pub fn shown_path(path: &Path) -> String {
    escape_controls(&path.display().to_string()).into_owned()
}

/// Print each realignment that a file already holding the new name prevented,
/// as a warning on stderr. The note itself is fine and keeps its old name.
pub fn print_refused_renames(refused: &[Rename]) {
    for rename in refused {
        eprintln!(
            "warning: did not rename `{}` to `{}`: a file of that name already exists",
            file_name(&rename.from),
            file_name(&rename.to)
        );
    }
}

/// One line telling that a copied note got a ULID of its own, and which file
/// kept the one it shared (ADR 0067).
pub fn reassignment(reassignment: &Reassignment) -> String {
    format!(
        "reassigned {} -> {}, its id stays with {}",
        file_name(&reassignment.from),
        file_name(&reassignment.to),
        file_name(&reassignment.keeper)
    )
}

/// Report on stderr what realigning an edited or written note did besides
/// renaming it: copies given their own ULID, and a rename it could not make.
///
/// stderr, so `write`'s stdout stays the one path a script reads.
pub fn print_realignment(realignment: &Realignment) {
    for reassigned in &realignment.reassigned {
        eprintln!("note: {}", reassignment(reassigned));
    }
    print_refused_renames(realignment.refused.as_slice());
}

/// The file-name component of a path as a lossy string, or the whole path
/// when it has none, escaped for a message (ADR 0071).
pub fn file_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    escape_controls(&name).into_owned()
}

/// A byte count as a short human-readable figure: whole bytes below a KiB,
/// otherwise one decimal in binary units, matching the sizes `ls -lh` shows.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64 / 1024.0;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
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
///
/// Cells hold text from the vault and are escaped once, up front (ADR 0071),
/// so the widths are measured on the same strings that are written.
fn write_table(out: &mut impl Write, header: &[&str], rows: &[Vec<String>]) -> io::Result<()> {
    let rows: Vec<Vec<Cow<'_, str>>> = rows
        .iter()
        .map(|row| row.iter().map(|cell| escape_controls(cell)).collect())
        .collect();
    let widths = column_widths(header, &rows);
    write_row(out, header.iter().copied(), &widths)?;
    for row in &rows {
        write_row(out, row.iter().map(|cell| cell.as_ref()), &widths)?;
    }
    Ok(())
}

/// The display-column width of each column: the widest of the header cell and
/// every row's cell in that position.
fn column_widths(header: &[&str], rows: &[Vec<Cow<'_, str>>]) -> Vec<usize> {
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
    fn human_size_picks_the_binary_unit() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(9), "9 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(12_700), "12.4 KiB");
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024 / 2), "1.5 GiB");
        assert_eq!(human_size(u64::MAX), "16777216.0 TiB");
    }

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

    // -------------------------------------------------------------------------
    // Text from the vault is escaped before it is shown (ADR 0071)
    // -------------------------------------------------------------------------

    #[test]
    fn a_line_break_in_a_cell_keeps_the_row_on_one_line() {
        let out = render(&["TITLE", "PATH"], &[row(&["a\nb", "a.md"])]);
        assert_eq!(out.lines().count(), 2, "{out}");
    }

    #[test]
    fn a_tab_in_a_cell_is_escaped() {
        let out = render(&["TITLE", "PATH"], &[row(&["a\tb", "a.md"])]);
        assert!(!out.contains('\t'), "{out:?}");
        assert!(out.contains("a\\tb"), "{out:?}");
    }

    #[test]
    fn a_control_character_cannot_reach_the_table() {
        let out = render(
            &["TITLE", "PATH"],
            &[row(&["Evil\u{1b}]52;c;ZXZpbAo=\u{7}", "a.md"])],
        );
        assert!(!out.contains('\u{1b}'), "{out:?}");
        assert!(!out.contains('\u{7}'), "{out:?}");
    }

    #[test]
    fn a_path_cell_is_escaped() {
        let out = render(
            &["TITLE", "PATH"],
            &[row(&["t", "/v/all-notes/x\u{1b}y.md"])],
        );
        assert!(out.contains("/v/all-notes/x\\u{1b}y.md"), "{out:?}");
    }

    #[test]
    fn cells_are_measured_after_escaping() {
        // `a\u{1b}` is seven columns once escaped, the widest cell, so the
        // other rows pad to it.
        let out = render(
            &["T", "P"],
            &[row(&["a\u{1b}", "z"]), row(&["bbbbbbb", "z"])],
        );
        assert_eq!(out, "T        P\na\\u{1b}  z\nbbbbbbb  z\n");
    }

    #[test]
    fn reference_escapes_the_title() {
        let r = reference(ULID, "2026-06-25", "a\u{1b}[2Jb", &[]);
        assert_eq!(r, "2026-06-25  a\\u{1b}[2Jb  (01ARZ3NDEKTSV4RRFFQ69G5FAV)");
    }

    #[test]
    fn a_warning_escapes_the_file_name() {
        let warning = ScanWarning {
            path: std::path::PathBuf::from(format!("/v/all-notes/{ULID}-\u{1b}x.md")),
            message: "the frontmatter is not valid YAML".to_string(),
        };
        assert_eq!(
            format_warning(&warning),
            format!("skipped `{ULID}-\\u{{1b}}x.md`: the frontmatter is not valid YAML")
        );
    }

    #[test]
    fn a_warning_escapes_the_message() {
        let warning = ScanWarning {
            path: std::path::PathBuf::from("/v/all-notes/x.md"),
            message: "`\u{1b}x.md` does not start with a valid ULID".to_string(),
        };
        assert_eq!(
            format_warning(&warning),
            "skipped `x.md`: `\\u{1b}x.md` does not start with a valid ULID"
        );
    }

    #[test]
    fn a_reassignment_line_escapes_every_name() {
        let line = reassignment(&Reassignment {
            from: format!("/v/all-notes/{ULID}-\u{1b}copy.md").into(),
            to: "/v/all-notes/01BRZ3NDEKTSV4RRFFQ69G5FAV-copy.md".into(),
            keeper: format!("/v/all-notes/{ULID}-\u{202e}plan.md").into(),
        });
        assert_eq!(
            line,
            format!(
                "reassigned {ULID}-\\u{{1b}}copy.md -> 01BRZ3NDEKTSV4RRFFQ69G5FAV-copy.md, \
                 its id stays with {ULID}-\\u{{202e}}plan.md"
            )
        );
    }

    #[test]
    fn an_error_message_is_escaped() {
        let error = anyhow::anyhow!("`x\u{1b}y.md` is not a note").context("while writing");
        assert_eq!(
            error_message(&error),
            "while writing: `x\\u{1b}y.md` is not a note"
        );
    }

    #[test]
    fn an_error_message_keeps_its_line_breaks() {
        let error =
            anyhow::anyhow!("these views cannot be removed safely:\n  view `a\u{1b}b`: invalid");
        let message = error_message(&error);
        assert_eq!(message.lines().count(), 2, "{message}");
        assert!(!message.contains('\u{1b}'), "{message:?}");
    }

    #[test]
    fn a_tab_in_a_tools_output_is_escaped() {
        let error = anyhow::anyhow!("`typst` failed:\nerror: unknown\n\tat line 1");
        assert_eq!(
            error_message(&error),
            "`typst` failed:\nerror: unknown\n\\tat line 1"
        );
    }

    #[test]
    fn a_colour_sequence_in_a_tools_output_is_escaped() {
        let error = anyhow::anyhow!("`typst` failed:\n\u{1b}[31merror\u{1b}[0m: unknown");
        assert_eq!(
            error_message(&error),
            "`typst` failed:\n\\u{1b}[31merror\\u{1b}[0m: unknown"
        );
    }

    #[test]
    fn a_clean_path_is_printed_as_it_is() {
        let path = Path::new("/v/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-plan.md");
        assert_eq!(checked_path(path, true).expect("a clean path prints"), path);
    }

    #[test]
    fn a_note_path_that_needs_escaping_is_refused() {
        let path = Path::new("/v/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-\u{1b}x.md");
        let error = checked_path(path, true).expect_err("an escaped path is not printed");
        assert_eq!(
            error.to_string(),
            "`/v/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-\\u{1b}x.md` holds control or bidi \
             characters, so it is not printed; run `ntropy reconcile` to rename the note"
        );
    }

    #[test]
    fn another_path_that_needs_escaping_is_refused_without_the_reconcile_hint() {
        let path = Path::new("/v\u{1b}/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-plan.md");
        let error = checked_path(path, false).expect_err("an escaped path is not printed");
        assert_eq!(
            error.to_string(),
            "`/v\\u{1b}/all-notes/01ARZ3NDEKTSV4RRFFQ69G5FAV-plan.md` holds control or bidi \
             characters, so it is not printed"
        );
    }
}
