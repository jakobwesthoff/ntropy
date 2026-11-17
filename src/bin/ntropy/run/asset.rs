// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `ntropy asset add` and `ntropy asset list` (ADR 0104).
//!
//! Both work on exactly one note, selected as `render` and `delete` select
//! theirs: a ULID, a file name or path, or a query, with the picker on a
//! terminal when several match and an error under `-n`. Nothing is ever put
//! into several notes, and the note itself is never edited: `add` prints the
//! link and the caller places it.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use ntropy::asset::{self, AddOptions};
use ntropy::ops::Selector;
use ntropy::reconcile;
use ntropy::session::VaultSession;

use super::one_note::{Selection, select_one};
use super::{exit_for_warnings, output, print_skipped_views};
use crate::cli::{AssetCommand, GlobalArgs};

pub(super) fn cmd_asset(
    global: &GlobalArgs,
    session: &VaultSession,
    command: AssetCommand,
    interactive: bool,
) -> Result<ExitCode> {
    // The files of an encrypted vault's notes are encrypted and the assets
    // are not, so the commands would pile plaintext beside them (ADR 0103).
    if session.is_encrypted() {
        bail!(
            "asset encryption is not supported yet: `ntropy asset` does not work in an \
             encrypted vault"
        );
    }
    match command {
        AssetCommand::Add {
            move_files,
            rename,
            force,
            selector,
            files,
        } => cmd_add(
            global,
            session,
            Selector::from_arguments(&[selector]),
            &files,
            &AddOptions {
                move_files,
                rename,
                force,
            },
            interactive,
        ),
        AssetCommand::List { path, selector } => cmd_list(
            global,
            session,
            Selector::from_arguments(&selector),
            path,
            interactive,
        ),
    }
}

fn cmd_add(
    global: &GlobalArgs,
    session: &VaultSession,
    selector: Selector,
    files: &[PathBuf],
    options: &AddOptions,
    interactive: bool,
) -> Result<ExitCode> {
    let (selection, warnings) =
        select_one(session, &selector, interactive, false, "adding assets")?;
    let note = match selection {
        Selection::Note(note) => note,
        Selection::Exit(code) => return Ok(code),
    };

    let report = asset::add(session.layout(), &note, files, options);

    // What was placed is shown first, whatever came after: a file that moved
    // is not lost track of because a later one failed.
    for file in &report.added {
        eprintln!("added {}", shown_in_vault(session, &file.path));
    }
    for file in &report.added {
        println!("{}", ntropy::text::escape_controls(&file.link));
    }

    // The views follow a folder that appeared or a file that arrived. A view
    // that cannot be refreshed is a warning, since the files are in place and
    // the links are printed.
    let mut extra = 0;
    if report.touched {
        match reconcile::refresh_views(session) {
            Ok(refresh) => print_skipped_views(&refresh.skipped_views),
            Err(error) => {
                output::warn(&format!("could not refresh the views: {error:#}"));
                extra += 1;
            }
        }
    }

    if let Some(error) = report.error {
        return Err(error).context("while adding the assets");
    }
    Ok(exit_for_warnings(global.strict, &warnings, extra))
}

fn cmd_list(
    global: &GlobalArgs,
    session: &VaultSession,
    selector: Selector,
    path_only: bool,
    interactive: bool,
) -> Result<ExitCode> {
    let (selection, warnings) =
        select_one(session, &selector, interactive, path_only, "listing assets")?;
    let note = match selection {
        Selection::Note(note) => note,
        Selection::Exit(code) => return Ok(code),
    };

    let listing = asset::list(session.layout(), &note).context("while listing the assets")?;
    if path_only {
        println!(
            "{}",
            output::checked_path(&listing.folder, false)?.display()
        );
    } else if listing.files.is_empty() {
        eprintln!("no assets");
    } else {
        output::print_asset_listing(&listing)?;
    }
    Ok(exit_for_warnings(global.strict, &warnings, 0))
}

/// `path` as the vault shows it, `all-notes/_assets/<folder>/<file>`, escaped
/// for a message (ADR 0071).
fn shown_in_vault(session: &VaultSession, path: &Path) -> String {
    let relative = path.strip_prefix(session.root()).unwrap_or(path);
    ntropy::text::escape_controls(&relative.to_string_lossy()).into_owned()
}
