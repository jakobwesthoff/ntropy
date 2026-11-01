// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Narrowing a selector to the one note a command works on (ADR 0025,
//! ADR 0037).
//!
//! `render` and `show` act on exactly one note. A blank selector browses the
//! whole vault as `search` does; anything else resolves as a full ULID or a
//! DSL query. Several matches open the picker interactively and are an error
//! under `-n`.

use std::process::ExitCode;

use anyhow::{Context, Result};

use ntropy::note::Note;
use ntropy::ops;
use ntropy::scan::ScanWarning;
use ntropy::session::VaultSession;

use super::{output, picker, report_ambiguous};

/// What the selection came to: the note to work on, or the exit code the
/// command ends with because there is none.
pub(super) enum Selection {
    Note(Box<Note>),
    Exit(ExitCode),
}

/// Resolve `selector` to one note of `session`.
///
/// The scan's warnings are printed here and returned, so the caller can fold
/// them into its `--strict` exit. `print` is the caller's `-p`: a cancelled
/// picker then fails, so `open "$(ntropy render -p ...)"` branches correctly;
/// without it a cancel is a successful no-op like `delete`. `verb` names the
/// command's work in the error a bare non-interactive invocation gets, as in
/// "rendering needs a selector".
pub(super) fn select_one(
    session: &VaultSession,
    selector: &str,
    interactive: bool,
    print: bool,
    verb: &str,
) -> Result<(Selection, Vec<ScanWarning>)> {
    let selector = super::optional(selector).map(str::to_string);
    let matches = match selector.as_deref() {
        Some(selector) => {
            ops::resolve_selection(session, selector).context("while resolving the selector")?
        }
        None => ops::search(session, None).context("while listing notes")?,
    };
    output::print_warnings(&matches.warnings);

    // The picker yields a candidate, so the full note is recovered from the
    // resolved set by id, since the command needs its body.
    let selection = match matches.notes.as_slice() {
        [] => {
            match &selector {
                Some(selector) => eprintln!("error: no note matches `{selector}`"),
                None => eprintln!("No notes matched your search criteria."),
            }
            Selection::Exit(ExitCode::FAILURE)
        }
        [note] => Selection::Note(Box::new(note.clone())),
        notes => {
            if interactive {
                let candidates = ops::to_candidates(notes)?;
                match picker::pick(candidates, picker::align_candidates)? {
                    Some(selected) => Selection::Note(Box::new(
                        notes
                            .iter()
                            .find(|n| n.id == selected.id)
                            .expect("the picked candidate is one of the resolved notes")
                            .clone(),
                    )),
                    None if print => Selection::Exit(ExitCode::FAILURE),
                    None => Selection::Exit(ExitCode::SUCCESS),
                }
            } else {
                match &selector {
                    Some(selector) => report_ambiguous(selector, notes)?,
                    // Without a picker there is nothing to narrow a bare
                    // invocation with, so it asks for a selector instead of
                    // dumping every note as an ambiguity list.
                    None => eprintln!(
                        "error: {verb} needs a selector in non-interactive mode ({} notes)",
                        notes.len()
                    ),
                }
                Selection::Exit(ExitCode::FAILURE)
            }
        }
    };
    Ok((selection, matches.warnings))
}
