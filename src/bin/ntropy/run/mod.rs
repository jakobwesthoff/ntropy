// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Dispatch: parsed CLI to library use cases (`docs/design/cli.md`).
//!
//! This layer owns everything the headless library deliberately does not: vault
//! resolution from flags/env/config, the interactive-vs-plain choice, the
//! picker and editor, confirmation prompts, and translating outcomes into exit
//! codes. Each command resolves to one or more `ops::` calls plus presentation.

mod editor;
mod interact;
mod output;
mod picker;

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::CommandFactory;

use ntropy::config::global;
use ntropy::ops;
use ntropy::reconcile;
use ntropy::scan::ScanWarning;
use ntropy::vault::{ResolveOptions, Vault};

use crate::cli::{Cli, Command, GlobalArgs, ViewCommand, join};

/// Run the parsed CLI to completion, returning the process exit code.
pub fn run(cli: Cli) -> Result<ExitCode> {
    let Some(command) = cli.command else {
        // Bare `ntropy` prints help (ADR 0018).
        Cli::command().print_help()?;
        println!();
        return Ok(ExitCode::SUCCESS);
    };

    // `init` is the one command that does not operate on an already-resolved
    // vault, so it is handled before resolution.
    if let Command::Init { path, set_default } = command {
        return cmd_init(path, set_default);
    }

    let vault = resolve_vault(&cli.global)?;
    let interactive = interact::is_interactive(cli.global.non_interactive);

    match command {
        // Handled above, before vault resolution.
        Command::Init { .. } => unreachable!("init is dispatched before vault resolution"),
        Command::Search { query } => cmd_search(&cli.global, &vault, join(&query), interactive),
        Command::New { title, no_edit } => cmd_new(&vault, join(&title), no_edit, interactive),
        Command::Edit { selector } => cmd_edit(&vault, join(&selector), interactive),
        Command::Reconcile => cmd_reconcile(&cli.global, &vault),
        Command::Delete { selector, force } => {
            cmd_delete(&vault, join(&selector), force, interactive)
        }
        Command::View(sub) => cmd_view(&vault, sub),
        Command::Tags => cmd_tags(&cli.global, &vault),
    }
}

// =============================================================================
// Vault resolution
// =============================================================================

fn resolve_vault(global: &GlobalArgs) -> Result<Vault> {
    let global_default = global::load()
        .context("while loading the global config")?
        .default_vault;
    let opts = ResolveOptions {
        explicit: global.vault.clone(),
        env: std::env::var_os("NTROPY_VAULT").map(PathBuf::from),
        start_dir: std::env::current_dir().ok(),
        global_default,
    };
    Vault::resolve(&opts).context("while resolving the vault")
}

// =============================================================================
// Commands
// =============================================================================

fn cmd_init(path: Option<PathBuf>, set_default: bool) -> Result<ExitCode> {
    let target = match path {
        Some(p) => p,
        None => std::env::current_dir().context("while reading the current directory")?,
    };
    let report = ops::init_vault(&target).context("while initializing the vault")?;

    if report.created.is_empty() {
        println!("Vault already initialized at {}", report.root.display());
    } else {
        println!("Initialized vault at {}", report.root.display());
    }

    if set_default {
        let canonical = std::fs::canonicalize(&report.root).unwrap_or_else(|_| report.root.clone());
        set_global_default(&canonical)?;
        println!("Set as default vault.");
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_search(
    global: &GlobalArgs,
    vault: &Vault,
    query: String,
    interactive: bool,
) -> Result<ExitCode> {
    let query = optional(&query);
    let matches = ops::search(vault, query).context("while searching")?;
    output::print_warnings(&matches.warnings);

    if interactive {
        let candidates = ops::to_candidates(&matches.notes)?;
        if let Some(selected) = picker::pick(candidates, picker::render_candidate)? {
            open_and_refresh(vault, &selected.path)?;
        }
    } else {
        output::print_notes(&matches.notes)?;
    }
    Ok(exit_for_warnings(global.strict, &matches.warnings))
}

fn cmd_new(vault: &Vault, title: String, no_edit: bool, interactive: bool) -> Result<ExitCode> {
    let note = ops::create_note(vault, &title).context("while creating the note")?;

    // Open the editor only when interactive and not explicitly suppressed;
    // otherwise create-and-print for scripting (ADR 0015).
    if !no_edit && interactive {
        open_and_refresh(vault, &note.path)?;
    } else {
        reconcile::refresh_views(vault).context("while refreshing views")?;
        println!("{}", note.path.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_edit(vault: &Vault, selector: String, interactive: bool) -> Result<ExitCode> {
    let matches =
        ops::resolve_selection(vault, &selector).context("while resolving the selector")?;
    output::print_warnings(&matches.warnings);

    match matches.notes.as_slice() {
        [] => {
            eprintln!("error: no note matches `{selector}`");
            Ok(ExitCode::FAILURE)
        }
        [note] => {
            open_and_refresh(vault, &note.path)?;
            Ok(ExitCode::SUCCESS)
        }
        notes => {
            if interactive {
                let candidates = ops::to_candidates(notes)?;
                if let Some(selected) = picker::pick(candidates, picker::render_candidate)? {
                    open_and_refresh(vault, &selected.path)?;
                }
                Ok(ExitCode::SUCCESS)
            } else {
                report_ambiguous(&selector, notes);
                Ok(ExitCode::FAILURE)
            }
        }
    }
}

fn cmd_reconcile(global: &GlobalArgs, vault: &Vault) -> Result<ExitCode> {
    let report = reconcile::reconcile(vault).context("while reconciling")?;
    output::print_warnings(&report.warnings);
    for rename in &report.renamed {
        println!(
            "renamed {} -> {}",
            file_name(&rename.from),
            file_name(&rename.to)
        );
    }
    Ok(exit_for_warnings(global.strict, &report.warnings))
}

fn cmd_delete(vault: &Vault, selector: String, force: bool, interactive: bool) -> Result<ExitCode> {
    let matches =
        ops::resolve_selection(vault, &selector).context("while resolving the selector")?;
    output::print_warnings(&matches.warnings);

    // Determine the single target note (path + title), honoring the ambiguity
    // rule shared with `edit` (ADR 0025).
    let target = match matches.notes.as_slice() {
        [] => {
            eprintln!("error: no note matches `{selector}`");
            return Ok(ExitCode::FAILURE);
        }
        [note] => (note.path.clone(), note.title.clone()),
        notes => {
            if interactive {
                let candidates = ops::to_candidates(notes)?;
                match picker::pick(candidates, picker::render_candidate)? {
                    Some(selected) => (selected.path, selected.title),
                    None => return Ok(ExitCode::SUCCESS),
                }
            } else {
                report_ambiguous(&selector, notes);
                return Ok(ExitCode::FAILURE);
            }
        }
    };
    let (path, title) = target;

    if !force {
        if !interactive {
            bail!("refusing to delete `{title}` without --force in non-interactive mode");
        }
        if !confirm(&format!("Delete \"{title}\"? [y/N] "))? {
            println!("Aborted.");
            return Ok(ExitCode::SUCCESS);
        }
    }

    // The resolution scan above already surfaced any warnings; the rebuild
    // scans the same vault, so its warnings are discarded to avoid printing
    // each one twice.
    ops::delete_note(vault, &path).context("while deleting the note")?;
    println!("Deleted {}", file_name(&path));
    Ok(ExitCode::SUCCESS)
}

fn cmd_view(vault: &Vault, sub: ViewCommand) -> Result<ExitCode> {
    match sub {
        ViewCommand::List => {
            let views = ops::list_views(vault).context("while listing views")?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            for view in views {
                writeln!(out, "{}\t{}", view.name, view.field)?;
            }
        }
        ViewCommand::Add { name, field } => {
            ops::add_view(vault, &name, &field).context("while adding the view")?;
            println!("Added view `{name}` (field `{field}`).");
        }
        ViewCommand::Remove { name } => {
            ops::remove_view(vault, &name).context("while removing the view")?;
            println!("Removed view `{name}`.");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_tags(global: &GlobalArgs, vault: &Vault) -> Result<ExitCode> {
    let list = ops::list_tags(vault).context("while listing tags")?;
    output::print_warnings(&list.warnings);
    output::print_tags(&list.tags)?;
    Ok(exit_for_warnings(global.strict, &list.warnings))
}

// =============================================================================
// Shared helpers
// =============================================================================

/// Open a note in the editor, then realign its filename and rebuild views.
///
/// Only the touched note is realigned, so an out-of-band drift elsewhere is
/// never renamed silently (ADR 0004); the view rebuild then reflects any title
/// or tag change made during the edit.
fn open_and_refresh(vault: &Vault, path: &Path) -> Result<()> {
    editor::open(path)?;
    reconcile::realign(path).context("while realigning the edited note")?;
    reconcile::refresh_views(vault).context("while refreshing views")?;
    Ok(())
}

/// Write the default-vault entry to the global config.
fn set_global_default(root: &Path) -> Result<()> {
    let path = global::config_path()
        .ok_or_else(|| anyhow!("no global config directory is available on this system"))?;
    let mut config = global::load_at(&path).unwrap_or_default();
    config.default_vault = Some(root.to_path_buf());
    global::write_at(&path, &config).context("while writing the global config")?;
    Ok(())
}

/// Print an "ambiguous selector" error and the candidate notes to stderr.
fn report_ambiguous(selector: &str, notes: &[ntropy::note::Note]) {
    eprintln!(
        "error: `{selector}` is ambiguous ({} matches):",
        notes.len()
    );
    for note in notes {
        eprintln!("  {}\t{}", note.id, note.title);
    }
    eprintln!("refine the query or pass a full ULID");
}

/// Prompt on stdin for a yes/no confirmation.
fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// `None` for a blank query string, otherwise the trimmed query.
fn optional(query: &str) -> Option<&str> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// The exit code for a command that ran but hit scan warnings under `--strict`.
fn exit_for_warnings(strict: bool, warnings: &[ScanWarning]) -> ExitCode {
    if strict && !warnings.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The file-name component of a path as a lossy string.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
