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
mod lsp;
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
use ntropy::vault::{ResolveOptions, Vault, resolve};

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
        return cmd_init(path, cli.global.vault.clone(), set_default);
    }

    // `info` resolves the vault itself so it can report which rule matched.
    if let Command::Info = command {
        return cmd_info(&cli.global);
    }

    // The language server resolves a vault per document, so it starts without
    // one (ADR 0029).
    if let Command::Lsp = command {
        return lsp::run();
    }

    let vault = resolve_vault(&cli.global)?;
    let interactive = interact::is_interactive(cli.global.non_interactive);

    match command {
        // Handled above, before vault resolution.
        Command::Init { .. } => unreachable!("init is dispatched before vault resolution"),
        Command::Search { query } => cmd_search(&cli.global, &vault, join(&query), interactive),
        Command::New {
            title,
            template,
            no_edit,
        } => cmd_new(&vault, join(&title), template, no_edit, interactive),
        Command::Today { no_edit } => cmd_today(&vault, no_edit, interactive),
        Command::Reconcile => cmd_reconcile(&cli.global, &vault),
        Command::Delete { selector, force } => {
            cmd_delete(&vault, join(&selector), force, interactive)
        }
        Command::View(sub) => cmd_view(&vault, sub),
        Command::Tags => cmd_tags(&cli.global, &vault),
        // Handled above, before vault resolution.
        Command::Info => unreachable!("info is dispatched before vault resolution"),
        Command::Lsp => unreachable!("lsp is dispatched before vault resolution"),
    }
}

/// How many of the most-used tags `info` reports.
const TOP_TAGS: usize = 5;

// =============================================================================
// Vault resolution
// =============================================================================

fn resolve_options(global: &GlobalArgs) -> Result<ResolveOptions> {
    let global_default = global::load()
        .context("while loading the global config")?
        .default_vault;
    Ok(ResolveOptions {
        explicit: global.vault.clone(),
        env: std::env::var_os("NTROPY_VAULT").map(PathBuf::from),
        start_dir: std::env::current_dir().ok(),
        global_default,
    })
}

fn resolve_vault(global: &GlobalArgs) -> Result<Vault> {
    let opts = resolve_options(global)?;
    Vault::resolve(&opts).context("while resolving the vault")
}

// =============================================================================
// Commands
// =============================================================================

fn cmd_init(path: Option<PathBuf>, vault: Option<PathBuf>, set_default: bool) -> Result<ExitCode> {
    // The positional path and the global `--vault` are two ways to name the same
    // target, so requiring exactly one keeps the destination unambiguous. With
    // neither, `init` scaffolds the current directory.
    let target = match (path, vault) {
        (Some(_), Some(_)) => {
            bail!("pass the target as either `--vault` or the positional path, not both")
        }
        (Some(p), None) => p,
        (None, Some(v)) => v,
        (None, None) => std::env::current_dir().context("while reading the current directory")?,
    };
    let report = ops::init_vault(&target).context("while initializing the vault")?;

    let touched_gitignore =
        !report.gitignore_added.is_empty() || !report.gitignore_removed.is_empty();
    if report.created.is_empty() && !touched_gitignore {
        println!("Vault already initialized at {}", report.root.display());
    } else {
        println!("Initialized vault at {}", report.root.display());
    }
    report_gitignore_changes(&report.gitignore_added, &report.gitignore_removed);

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
    selector: String,
    interactive: bool,
) -> Result<ExitCode> {
    // A bare invocation browses the whole vault; a selector resolves a full ULID
    // directly or otherwise runs as a DSL query (the id-or-query rule of ADR
    // 0025). Both feed one presentation path, which `edit` shares verbatim as a
    // hidden alias (ADR 0031).
    let matches = match optional(&selector) {
        Some(selector) => {
            ops::resolve_selection(vault, selector).context("while resolving the selector")?
        }
        None => ops::search(vault, None).context("while listing notes")?,
    };
    output::print_warnings(&matches.warnings);

    match matches.notes.as_slice() {
        // A no-match, including an empty-vault listing, is a non-zero exit so
        // `search <x> && ...` branches correctly. The message goes to stderr to
        // keep stdout clean for pipelines.
        [] => {
            eprintln!("No notes matched your search criteria.");
            Ok(ExitCode::FAILURE)
        }
        notes => {
            if interactive {
                // On a TTY a lone match opens straight away; several open the
                // picker pre-filtered to them.
                if let [note] = notes {
                    open_and_refresh(vault, &note.path)?;
                } else {
                    let candidates = ops::to_candidates(notes)?;
                    if let Some(selected) = picker::pick(candidates, picker::align_candidates)? {
                        open_and_refresh(vault, &selected.path)?;
                    }
                }
            } else {
                // Without a TTY the editor never opens, mirroring `new`/`today`
                // (ADR 0015); the plain table is printed instead.
                output::print_notes(notes)?;
            }
            Ok(exit_for_warnings(global.strict, &matches.warnings))
        }
    }
}

fn cmd_new(
    vault: &Vault,
    title: String,
    template: Option<String>,
    no_edit: bool,
    interactive: bool,
) -> Result<ExitCode> {
    let note =
        ops::create_note(vault, &title, template.as_deref()).context("while creating the note")?;

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

fn cmd_today(vault: &Vault, no_edit: bool, interactive: bool) -> Result<ExitCode> {
    let outcome = ops::today_note(vault).context("while preparing today's note")?;

    // Mirror `new`: open interactively unless suppressed, otherwise refresh views
    // and print the path for scripting (ADR 0015).
    if !no_edit && interactive {
        open_and_refresh(vault, &outcome.note.path)?;
    } else {
        reconcile::refresh_views(vault).context("while refreshing views")?;
        println!("{}", outcome.note.path.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_reconcile(global: &GlobalArgs, vault: &Vault) -> Result<ExitCode> {
    println!("Reconciling vault at {}...", vault.root().display());
    let report = reconcile::reconcile(vault).context("while reconciling")?;
    output::print_warnings(&report.warnings);
    for rename in &report.renamed {
        println!(
            "renamed {} -> {}",
            file_name(&rename.from),
            file_name(&rename.to)
        );
    }
    for rewrite in &report.links_rewritten {
        println!(
            "relinked {} -> {} in {}",
            rewrite.from,
            rewrite.to,
            file_name(&rewrite.note)
        );
    }
    report_gitignore_changes(&report.gitignore_added, &report.gitignore_removed);
    // A summary always prints, so even a no-op run confirms what happened.
    println!(
        "Scanned {}, renamed {}, relinked {}, synced {}, ignored {}, unignored {}, {}.",
        plural(report.notes_scanned, "note", "notes"),
        plural(report.renamed.len(), "file", "files"),
        plural(report.links_rewritten.len(), "link", "links"),
        plural(report.views_synced, "view", "views"),
        plural(report.gitignore_added.len(), "entry", "entries"),
        plural(report.gitignore_removed.len(), "entry", "entries"),
        plural(report.warnings.len(), "warning", "warnings"),
    );
    Ok(exit_for_warnings(global.strict, &report.warnings))
}

fn cmd_delete(vault: &Vault, selector: String, force: bool, interactive: bool) -> Result<ExitCode> {
    let matches =
        ops::resolve_selection(vault, &selector).context("while resolving the selector")?;
    output::print_warnings(&matches.warnings);

    // Determine the single target note (path + human reference), honoring the
    // ambiguity rule shared with `edit` (ADR 0025).
    let target = match matches.notes.as_slice() {
        [] => {
            eprintln!("error: no note matches `{selector}`");
            return Ok(ExitCode::FAILURE);
        }
        [note] => (note.path.clone(), output::note_reference(note)?),
        notes => {
            if interactive {
                let candidates = ops::to_candidates(notes)?;
                match picker::pick(candidates, picker::align_candidates)? {
                    Some(selected) => {
                        let reference = output::reference(
                            selected.id,
                            &selected.date,
                            &selected.title,
                            &selected.tags,
                        );
                        (selected.path, reference)
                    }
                    None => return Ok(ExitCode::SUCCESS),
                }
            } else {
                report_ambiguous(&selector, notes)?;
                return Ok(ExitCode::FAILURE);
            }
        }
    };
    let (path, reference) = target;

    if !force {
        if !interactive {
            bail!("refusing to delete {reference} without --force in non-interactive mode");
        }
        if !confirm(&format!("Delete {reference}? [y/N] "))? {
            println!("Aborted.");
            return Ok(ExitCode::SUCCESS);
        }
    }

    // The resolution scan above already surfaced any warnings; the rebuild
    // scans the same vault, so its warnings are discarded to avoid printing
    // each one twice.
    ops::delete_note(vault, &path).context("while deleting the note")?;
    println!("Deleted {reference}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_view(vault: &Vault, sub: ViewCommand) -> Result<ExitCode> {
    match sub {
        ViewCommand::List => {
            let views = ops::list_views(vault).context("while listing views")?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            writeln!(out, "NAME\tFIELD")?;
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
            // ntropy never deletes the directory; tell the user it remains so
            // they can clean up the now-stale (and no longer ignored) tree.
            if vault.layout().view_dir(&name).exists() {
                println!(
                    "left directory `{name}/` in place, delete it manually if you no longer need it"
                );
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_info(global: &GlobalArgs) -> Result<ExitCode> {
    let opts = resolve_options(global)?;
    let (root, source) =
        resolve::resolve_with_source(&opts).context("while resolving the vault")?;
    let vault = Vault::new(root);
    let stats = ops::vault_stats(&vault, TOP_TAGS).context("while gathering vault info")?;
    output::print_info(&vault, &source, opts.global_default.as_deref(), &stats);
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

/// Print an "ambiguous selector" error and the candidate notes to stderr,
/// each as the shared human reference (ADR 0025).
fn report_ambiguous(selector: &str, notes: &[ntropy::note::Note]) -> Result<()> {
    eprintln!(
        "error: `{selector}` is ambiguous ({} matches):",
        notes.len()
    );
    for note in notes {
        eprintln!("  {}", output::note_reference(note)?);
    }
    eprintln!("refine the query or pass a full ULID");
    Ok(())
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

/// Format a count with its unit, choosing the singular or plural form.
fn plural(count: usize, singular: &str, plural: &str) -> String {
    let unit = if count == 1 { singular } else { plural };
    format!("{count} {unit}")
}

/// Report `.gitignore` changes, one line per entry.
///
/// A pruned entry also warns that the view's directory was *not* removed —
/// ntropy never deletes directories — so the user can clean it up themselves.
fn report_gitignore_changes(added: &[String], removed: &[String]) {
    for entry in added {
        println!("ignored {entry}");
    }
    for entry in removed {
        let dir = entry.trim_matches('/');
        println!(
            "stopped ignoring {entry}; left directory `{dir}/` in place, delete it manually if you no longer need it"
        );
    }
}

/// The file-name component of a path as a lossy string.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
