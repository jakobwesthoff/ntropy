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

use ntropy::asset::{self, AddOptions, BrokenLink};
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
        AssetCommand::List {
            path,
            folder,
            selector,
        } => {
            let form = match (path, folder) {
                (true, _) => ListForm::Files,
                (_, true) => ListForm::Folder,
                _ => ListForm::Table,
            };
            cmd_list(
                global,
                session,
                Selector::from_arguments(&selector),
                form,
                interactive,
            )
        }
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

    let mut extra = report.warnings.len();
    for warning in &report.warnings {
        output::warn(warning);
    }

    // The views follow a folder that appeared or a file that arrived. A view
    // that cannot be refreshed is a warning, since the files are in place and
    // the links are printed.
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

/// What `asset list` prints.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListForm {
    /// The table of files with size and link state.
    Table,
    /// The path of every asset file, for scripts.
    Files,
    /// The path of the folder, for scripts.
    Folder,
}

fn cmd_list(
    global: &GlobalArgs,
    session: &VaultSession,
    selector: Selector,
    form: ListForm,
    interactive: bool,
) -> Result<ExitCode> {
    // The script forms count as `-p` for the picker: a cancelled picker then
    // fails, so a script can tell it from a note without asset files, which
    // prints nothing and succeeds.
    let print = form != ListForm::Table;
    let (selection, warnings) =
        select_one(session, &selector, interactive, print, "listing assets")?;
    let note = match selection {
        Selection::Note(note) => note,
        Selection::Exit(code) => return Ok(code),
    };

    let listing = asset::list(session.layout(), &note).context("while listing the assets")?;

    // `--folder` prints a folder that may not exist yet, so that a script can
    // create it, and shows nothing about links. The other forms show the
    // links' outcome, and a link that leads nowhere counts for `--strict`.
    // The warnings come before the listing so that a form that fails to print
    // still reports them.
    let mut extra = 0;
    if form != ListForm::Folder {
        for file in &listing.files {
            if let Some(broken) = &file.broken {
                output::warn(&broken_link_warning(&listing.folder, broken));
                extra += 1;
            }
        }
    }

    match form {
        ListForm::Folder => println!(
            "{}",
            output::checked_path(&listing.folder, false)?.display()
        ),
        ListForm::Files => print_file_paths(&listing)?,
        ListForm::Table if listing.files.is_empty() => eprintln!("no assets"),
        ListForm::Table => output::print_asset_listing(&listing)?,
    }
    Ok(exit_for_warnings(global.strict, &warnings, extra))
}

/// The warning for a link the folder does not satisfy: the file is not in the
/// note's folder, or the link names a folder other than the note's.
fn broken_link_warning(folder: &Path, broken: &BrokenLink) -> String {
    if broken.names_own_folder {
        format!(
            "{} is linked but is not in the note's asset folder",
            broken.written
        )
    } else {
        let name = folder.file_name().unwrap_or_default().to_string_lossy();
        format!(
            "{} does not name the note's asset folder _assets/{name}/",
            broken.written
        )
    }
}

/// Print the path of every file of `listing`, one per line.
///
/// Nothing is printed unless every path can be: the listing decodes a name
/// that is not valid UTF-8 lossily, so it would print as a path that does not
/// exist, and a name with control characters would reach the terminal.
fn print_file_paths(listing: &asset::Listing) -> Result<()> {
    if let Some(name) = listing.non_utf8.first() {
        bail!(
            "`{}` is not valid UTF-8, so it is not printed",
            ntropy::text::escape_controls(name)
        );
    }
    let paths = listing.file_paths();
    for path in &paths {
        output::checked_path(path, false)?;
    }
    for path in &paths {
        println!("{}", path.display());
    }
    Ok(())
}

/// `path` as the vault shows it, `all-notes/_assets/<folder>/<file>`, escaped
/// for a message (ADR 0071).
fn shown_in_vault(session: &VaultSession, path: &Path) -> String {
    let relative = path.strip_prefix(session.root()).unwrap_or(path);
    ntropy::text::escape_controls(&relative.to_string_lossy()).into_owned()
}
