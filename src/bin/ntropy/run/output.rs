// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Plain, machine-friendly output (ADRs 0019, 0025).
//!
//! The non-interactive note table is `id<TAB>title<TAB>path`, one note per
//! line, no header, so `awk`/`cut` can split it. Scan warnings go to stderr by
//! file name (stdout stays clean for piping). Tags print as `tag<TAB>count`.

use std::io::Write;

use anyhow::Result;
use ntropy::note::Note;
use ntropy::ops::TagCount;
use ntropy::scan::ScanWarning;

/// Print notes as a tab-separated `id<TAB>title<TAB>path` table.
pub fn print_notes(notes: &[Note]) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for note in notes {
        writeln!(out, "{}\t{}\t{}", note.id, note.title, note.path.display())?;
    }
    Ok(())
}

/// Print tags as a tab-separated `tag<TAB>count` table.
pub fn print_tags(tags: &[TagCount]) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for entry in tags {
        writeln!(out, "{}\t{}", entry.tag, entry.count)?;
    }
    Ok(())
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
